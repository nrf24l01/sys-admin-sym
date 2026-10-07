//! Pixel work for background preparation tasks; never called while drawing UI.
use bevy::{
    asset::RenderAssetUsages, image::ImageSampler, prelude::*,
    render::render_resource::TextureFormat,
};
use std::sync::{Arc, OnceLock};

pub(super) struct Prepared {
    pub cpu: Option<Arc<Image>>,
    pub full: Image,
    pub thumbnail: Option<Image>,
    pub edge: u32,
}

pub(super) fn prepare(source: Image, edge: u32) -> Option<Prepared> {
    let mut full = prepare_image(&source)?;
    let thumbnail = (edge > 0).then(|| thumbnail(&full, edge)).flatten();
    let cpu = (edge > 0).then(|| Arc::new(full.clone()));
    full.asset_usage = RenderAssetUsages::RENDER_WORLD;
    Some(Prepared {
        cpu,
        full,
        thumbnail,
        edge,
    })
}

pub(super) fn thumbnail(source: &Image, edge: u32) -> Option<Image> {
    if source.width().max(source.height()) <= edge {
        return None;
    }
    let ratio = edge as f64 / f64::from(source.width().max(source.height()));
    let width = (f64::from(source.width()) * ratio).round().max(1.0) as u32;
    let height = (f64::from(source.height()) * ratio).round().max(1.0) as u32;
    let tables = color_tables();
    let linear: Vec<f32> = source
        .data
        .as_ref()?
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|pixel| {
            [
                tables.linear[usize::from(pixel[0])],
                tables.linear[usize::from(pixel[1])],
                tables.linear[usize::from(pixel[2])],
                f32::from(pixel[3]) / 255.0,
            ]
        })
        .collect();
    let pixels = image::Rgba32FImage::from_raw(source.width(), source.height(), linear)?;
    let resized = image::imageops::resize(
        &pixels,
        width,
        height,
        image::imageops::FilterType::Triangle,
    );
    let data = resized
        .pixels()
        .flat_map(|pixel| {
            let alpha = (pixel[3].clamp(0.0, 1.0) * 255.0).round() as u8;
            if alpha == 0 {
                [0; 4]
            } else {
                [
                    encode(pixel[0].clamp(0.0, pixel[3])),
                    encode(pixel[1].clamp(0.0, pixel[3])),
                    encode(pixel[2].clamp(0.0, pixel[3])),
                    alpha,
                ]
            }
        })
        .collect();
    let mut image = Image::new(
        bevy::render::render_resource::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::linear();
    Some(image)
}

struct ColorTables {
    linear: [f32; 256],
    premultiplied: Box<[u8]>,
}

fn color_tables() -> &'static ColorTables {
    static TABLES: OnceLock<ColorTables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let linear: [f32; 256] =
            std::array::from_fn(|value| Srgba::gamma_function(value as f32 / 255.0));
        let premultiplied = (0..65536)
            .map(|index| encode(linear[index % 256] * (index / 256) as f32 / 255.0))
            .collect();
        ColorTables {
            linear,
            premultiplied,
        }
    })
}

fn encode(linear: f32) -> u8 {
    (Srgba::gamma_function_inverse(linear) * 255.0).round() as u8
}

pub(super) fn prepare_image(source: &Image) -> Option<Image> {
    let mut image = if source.texture_descriptor.format == TextureFormat::Rgba8UnormSrgb {
        source.clone()
    } else {
        source.convert(TextureFormat::Rgba8UnormSrgb)?
    };
    let (width, height) = (image.width() as usize, image.height() as usize);
    let data = image.data.as_mut()?;
    if width == 0 || height == 0 || data.len() != width * height * 4 {
        return None;
    }
    let tables = color_tables();
    for pixel in data.as_chunks_mut::<4>().0 {
        let offset = usize::from(pixel[3]) * 256;
        for channel in &mut pixel[..3] {
            *channel = tables.premultiplied[offset + usize::from(*channel)];
        }
    }
    // Keep a single level: the mipmapped bindless texture path causes severe
    // rendering stalls on the NVIDIA Vulkan backend. Bilinear sampling with
    // premultiplied colors keeps transparent edges free of colored speckles.
    image.sampler = ImageSampler::linear();
    image.asset_usage = RenderAssetUsages::RENDER_WORLD;
    Some(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::render::render_resource::{Extent3d, TextureDimension};

    fn source(width: u32, height: u32, pixels: Vec<u8>) -> Image {
        Image::new(
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            pixels,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::MAIN_WORLD,
        )
    }

    #[test]
    fn transparent_rgb_is_removed_and_partial_alpha_is_premultiplied_in_linear_space() {
        let source = source(
            3,
            1,
            vec![255, 80, 20, 0, 255, 255, 255, 128, 32, 64, 128, 255],
        );
        let prepared = prepare_image(&source).unwrap();
        assert_eq!(
            &prepared.data.as_ref().unwrap()[..12],
            &[0, 0, 0, 0, 188, 188, 188, 128, 32, 64, 128, 255]
        );
        assert_eq!(&source.data.as_ref().unwrap()[..4], &[255, 80, 20, 0]);
    }

    #[test]
    fn prepared_texture_uses_single_level_bilinear_sampling() {
        let prepared = prepare_image(&source(2, 2, vec![255; 16])).unwrap();
        assert_eq!(prepared.texture_descriptor.mip_level_count, 1);
        assert_eq!(prepared.data.as_ref().unwrap().len(), 16);
        assert_eq!(prepared.sampler, ImageSampler::linear());
    }
    #[test]
    fn thumbnails_preserve_aspect_and_do_not_mix_hidden_transparent_colors() {
        let mut pixels = Vec::new();
        for pixel in 0..8 {
            pixels.extend(if pixel % 2 == 0 {
                [255; 4]
            } else {
                [255, 0, 0, 0]
            });
        }
        let full = prepare_image(&source(4, 2, pixels)).unwrap();
        let small = thumbnail(&full, 2).unwrap();
        assert_eq!((small.width(), small.height()), (2, 1));
        assert_eq!(small.texture_descriptor.mip_level_count, 1);
        for pixel in small.data.unwrap().as_chunks::<4>().0 {
            assert_eq!(pixel[0], pixel[1]);
            assert_eq!(pixel[1], pixel[2]);
            assert!(pixel[3] > 0 && pixel[3] < 255);
        }
        assert!(
            thumbnail(&full, 4).is_none(),
            "small sources reuse their full texture"
        );
    }
}
