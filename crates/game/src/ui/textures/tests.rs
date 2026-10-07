use super::*;
use bevy::{
    ecs::system::SystemState,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    tasks::TaskPoolBuilder,
};
use bevy_egui::EguiUserTextures;

fn source() -> Image {
    Image::new(
        Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        [255, 255, 255, 128].repeat(16),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD,
    )
}
fn wait_for(
    cache: &mut ImageTextures,
    assets: &mut Assets<Image>,
    ctx: &mut EguiContexts,
    edge: u32,
) {
    let started = std::time::Instant::now();
    while cache
        .prepared
        .values()
        .all(|image| !image.thumbnails.contains_key(&edge))
    {
        assert!(
            started.elapsed().as_secs() < 3,
            "image worker did not complete"
        );
        cache.update(assets, ctx, [], None);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
#[test]
fn frames_and_repeated_resizes_reuse_gpu_textures_and_decoded_pixels() {
    AsyncComputeTaskPool::get_or_init(|| TaskPoolBuilder::new().num_threads(2).build());
    let mut world = World::new();
    world.init_resource::<EguiUserTextures>();
    let mut system = SystemState::<EguiContexts>::new(&mut world);
    let mut ctx = system.get_mut(&mut world).unwrap();
    let mut assets = Assets::<Image>::default();
    let source = assets.add(source());
    let mut cache = ImageTextures::default();
    cache.track([(source.id(), true)]);
    cache.update(
        &mut assets,
        &mut ctx,
        [AssetEvent::Added { id: source.id() }],
        Some(2),
    );
    wait_for(&mut cache, &mut assets, &mut ctx, 2);
    assert!(
        assets.get(&source).unwrap().data.is_none(),
        "decoded bytes should have moved into the worker"
    );
    let cpu = cache.prepared[&source.id()].cpu.as_ref().unwrap().clone();
    let full = cache.texture(&source, &mut ctx);
    let thumbnail = cache.thumbnail(&source, &mut ctx);
    assert_ne!(full, thumbnail);
    let full_image = cache.prepared[&source.id()].full.image.clone();
    assets.get_mut_untracked(full_image.id()).unwrap().data = None;
    let count = assets.len();
    for _ in 0..100 {
        cache.update(&mut assets, &mut ctx, [], Some(2));
        assert_eq!(cache.texture(&source, &mut ctx), full);
        assert_eq!(cache.thumbnail(&source, &mut ctx), thumbnail);
        cache.retain_used(&mut ctx);
    }
    assert_eq!(assets.len(), count);
    assert!(cache.pending.is_empty() && cache.thumbnail_pending.is_empty());
    cache.update(&mut assets, &mut ctx, [], Some(3));
    wait_for(&mut cache, &mut assets, &mut ctx, 3);
    assert!(Arc::ptr_eq(
        &cpu,
        cache.prepared[&source.id()].cpu.as_ref().unwrap()
    ));
    let resized_count = assets.len();
    cache.update(&mut assets, &mut ctx, [], Some(2));
    assert_eq!(cache.thumbnail(&source, &mut ctx), thumbnail);
    assert_eq!(assets.len(), resized_count);
    assert!(cache.thumbnail_pending.is_empty());
}
#[test]
fn hidden_views_release_egui_bindings_but_reuse_uploaded_images_when_reopened() {
    AsyncComputeTaskPool::get_or_init(|| TaskPoolBuilder::new().num_threads(2).build());
    let mut world = World::new();
    world.init_resource::<EguiUserTextures>();
    let mut system = SystemState::<EguiContexts>::new(&mut world);
    let mut ctx = system.get_mut(&mut world).unwrap();
    let mut assets = Assets::<Image>::default();
    let source = assets.add(source());
    let mut cache = ImageTextures::default();
    cache.track([(source.id(), true)]);
    cache.update(
        &mut assets,
        &mut ctx,
        [AssetEvent::Added { id: source.id() }],
        Some(2),
    );
    wait_for(&mut cache, &mut assets, &mut ctx, 2);
    let full = cache.prepared[&source.id()].full.image.clone();
    let thumbnail = cache.prepared[&source.id()].thumbnails[&2].image.clone();
    let count = assets.len();
    cache.thumbnail(&source, &mut ctx);
    cache.retain_used(&mut ctx);
    assert!(cache.prepared[&source.id()].full.texture.is_none());
    assert!(
        cache.prepared[&source.id()].thumbnails[&2]
            .texture
            .is_some()
    );
    cache.retain_used(&mut ctx);
    assert!(
        cache.prepared[&source.id()].thumbnails[&2]
            .texture
            .is_none()
    );
    cache.texture(&source, &mut ctx);
    cache.thumbnail(&source, &mut ctx);
    cache.retain_used(&mut ctx);
    assert_eq!(cache.prepared[&source.id()].full.image.id(), full.id());
    assert_eq!(
        cache.prepared[&source.id()].thumbnails[&2].image.id(),
        thumbnail.id()
    );
    assert_eq!(assets.len(), count);
    assert!(cache.pending.is_empty() && cache.thumbnail_pending.is_empty());
}
#[test]
fn source_reload_replaces_the_cached_image_without_changing_original_png_semantics() {
    AsyncComputeTaskPool::get_or_init(|| TaskPoolBuilder::new().num_threads(2).build());
    let mut world = World::new();
    world.init_resource::<EguiUserTextures>();
    let mut system = SystemState::<EguiContexts>::new(&mut world);
    let mut ctx = system.get_mut(&mut world).unwrap();
    let mut assets = Assets::<Image>::default();
    let handle = assets.add(source());
    let mut cache = ImageTextures::default();
    cache.track([(handle.id(), true)]);
    cache.update(
        &mut assets,
        &mut ctx,
        [AssetEvent::Added { id: handle.id() }],
        Some(2),
    );
    wait_for(&mut cache, &mut assets, &mut ctx, 2);
    let previous = cache.prepared[&handle.id()].full.image.clone();
    let mut replacement = source();
    replacement.data.as_mut().unwrap().fill(255);
    assets.insert(handle.id(), replacement).unwrap();
    cache.update(
        &mut assets,
        &mut ctx,
        [AssetEvent::Modified { id: handle.id() }],
        None,
    );
    wait_for(&mut cache, &mut assets, &mut ctx, 2);
    assert_ne!(cache.prepared[&handle.id()].full.image.id(), previous.id());
    assert_eq!(
        &cache.prepared[&handle.id()]
            .cpu
            .as_ref()
            .unwrap()
            .data
            .as_ref()
            .unwrap()[..4],
        &[255; 4]
    );
}
