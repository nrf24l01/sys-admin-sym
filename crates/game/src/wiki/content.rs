use super::markdown::{MarkdownBlock, parse_markdown};
use bevy::prelude::Resource;
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

#[derive(Debug)]
pub(crate) struct WikiArticle {
    pub id: String,
    pub title: String,
    pub blocks: Vec<MarkdownBlock>,
}
#[derive(Resource)]
pub(crate) struct WikiLibrary {
    root: PathBuf,
    catalogs: BTreeMap<String, BTreeMap<String, WikiArticle>>,
    pub errors: Vec<String>,
}
impl WikiLibrary {
    pub fn load(root: &Path) -> Self {
        let mut library = Self {
            root: root.to_path_buf(),
            catalogs: BTreeMap::new(),
            errors: Vec::new(),
        };
        library.reload();
        library
    }
    pub fn reload(&mut self) {
        self.catalogs.clear();
        self.errors.clear();
        let directories = match std::fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) => {
                self.errors
                    .push(format!("{}: {error}", self.root.display()));
                return;
            }
        };
        let mut paths = Vec::new();
        for entry in directories {
            match entry {
                Ok(entry) => paths.push(entry.path()),
                Err(error) => self.errors.push(error.to_string()),
            }
        }
        paths.sort();
        for path in paths {
            if path
                .symlink_metadata()
                .is_ok_and(|m| m.is_dir() && !m.is_symlink())
            {
                let language = path.file_name().unwrap().to_string_lossy().into_owned();
                let mut catalog = BTreeMap::new();
                load_directory(&path, &path, &mut catalog, &mut self.errors);
                self.catalogs.insert(language, catalog);
            }
        }
    }
    pub fn article(&self, language: &str, id: &str) -> Option<&WikiArticle> {
        self.catalogs
            .get(language)
            .and_then(|c| c.get(id))
            .or_else(|| self.catalogs.get("en").and_then(|c| c.get(id)))
    }
    pub fn articles(&self, language: &str) -> Vec<&WikiArticle> {
        let mut merged = BTreeMap::new();
        for catalog in [self.catalogs.get("en"), self.catalogs.get(language)]
            .into_iter()
            .flatten()
        {
            merged.extend(catalog.iter().map(|(id, a)| (id.as_str(), a)));
        }
        merged.into_values().collect()
    }
}
fn load_directory(
    root: &Path,
    path: &Path,
    catalog: &mut BTreeMap<String, WikiArticle>,
    errors: &mut Vec<String>,
) {
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) => {
            errors.push(format!("{}: {error}", path.display()));
            return;
        }
    };
    let mut paths = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => paths.push(entry.path()),
            Err(error) => errors.push(error.to_string()),
        }
    }
    paths.sort();
    for path in paths {
        let metadata = match path.symlink_metadata() {
            Ok(m) => m,
            Err(error) => {
                errors.push(format!("{}: {error}", path.display()));
                continue;
            }
        };
        if metadata.is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            load_directory(root, &path, catalog, errors);
        } else if path.extension().is_some_and(|e| e == "md") {
            match std::fs::read_to_string(&path) {
                Ok(markdown) => {
                    let id = path
                        .strip_prefix(root)
                        .unwrap()
                        .with_extension("")
                        .to_string_lossy()
                        .replace('\\', "/");
                    let blocks = parse_markdown(&markdown);
                    let title = blocks
                        .iter()
                        .find_map(|b| match b {
                            MarkdownBlock::Heading {
                                level: 1, spans, ..
                            } => Some(spans.iter().map(|s| s.text.as_str()).collect()),
                            _ => None,
                        })
                        .unwrap_or_else(|| id.clone());
                    catalog.insert(id.clone(), WikiArticle { id, title, blocks });
                }
                Err(error) => errors.push(format!("{}: {error}", path.display())),
            }
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArticleLink {
    pub article: String,
    pub anchor: Option<String>,
}
pub(crate) fn resolve_article_link(from: &str, url: &str) -> Option<ArticleLink> {
    if url.starts_with('/') || url.starts_with('\\') || url.contains(':') || url.contains('\\') {
        return None;
    }
    let (target, anchor) = url
        .split_once('#')
        .map_or((url, None), |(p, a)| (p, Some(a.to_owned())));
    let mut parts: Vec<String> = if target.is_empty() {
        from.split('/').map(str::to_owned).collect()
    } else {
        from.rsplit_once('/').map_or(Vec::new(), |(p, _)| {
            p.split('/').map(str::to_owned).collect()
        })
    };
    for component in Path::new(target).components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?.to_owned()),
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::CurDir => {}
            _ => return None,
        }
    }
    let article = parts.join("/");
    if article.is_empty() {
        return None;
    }
    Some(ArticleLink {
        article: article.strip_suffix(".md").unwrap_or(&article).to_owned(),
        anchor,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn library_discovers_articles_falls_back_per_article_and_reloads() {
        let root = std::env::temp_dir().join(format!(
            "wiki-content-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("en/tutorial")).unwrap();
        std::fs::create_dir_all(root.join("ru/tutorial")).unwrap();
        std::fs::write(
            root.join("en/tutorial/first-vm.md"),
            "# First VM\n\nEnglish guide",
        )
        .unwrap();
        std::fs::write(root.join("en/network.md"), "# Network\n\nReference").unwrap();
        std::fs::write(
            root.join("ru/tutorial/first-vm.md"),
            "# Первая ВМ\n\nРуководство",
        )
        .unwrap();
        let mut library = WikiLibrary::load(&root);
        assert_eq!(
            library.article("ru", "tutorial/first-vm").unwrap().title,
            "Первая ВМ"
        );
        assert_eq!(library.article("ru", "network").unwrap().title, "Network");
        assert_eq!(library.articles("ru").len(), 2);
        std::fs::write(root.join("en/network.md"), "# Updated\n\nNew text").unwrap();
        library.reload();
        assert_eq!(library.article("en", "network").unwrap().title, "Updated");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn article_links_resolve_relative_paths_and_cannot_escape_the_library() {
        assert_eq!(
            resolve_article_link("tutorial/first-vm", "../reference/network.md#layer-2"),
            Some(ArticleLink {
                article: "reference/network".into(),
                anchor: Some("layer-2".into())
            })
        );
        assert_eq!(
            resolve_article_link("tutorial/first-vm", "#next"),
            Some(ArticleLink {
                article: "tutorial/first-vm".into(),
                anchor: Some("next".into())
            })
        );
        for url in [
            "../../secret.md",
            "/etc/passwd",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://example.com",
        ] {
            assert_eq!(
                resolve_article_link("tutorial/first-vm", url),
                None,
                "{url}"
            );
        }
    }

    #[test]
    fn bundled_lessons_have_both_languages_and_resolvable_links() {
        let library = WikiLibrary::load(&crate::asset_root().join("wiki"));
        assert!(library.errors.is_empty(), "{:?}", library.errors);
        for language in ["en", "ru"] {
            let articles = library.articles(language);
            assert!(articles.len() >= 10);
            for article in articles {
                assert!(!article.title.is_empty());
                for block in &article.blocks {
                    for span in block.spans() {
                        if let Some(url) = &span.link {
                            let target = resolve_article_link(&article.id, url).unwrap();
                            let linked = library
                                .article(language, &target.article)
                                .unwrap_or_else(|| panic!("{} links to missing {url}", article.id));
                            if let Some(anchor) = target.anchor {
                                assert!(linked.blocks.iter().any(|block| matches!(block, MarkdownBlock::Heading { anchor: a, .. } if a == &anchor)), "Missing anchor {anchor}");
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(
            library.catalogs["en"].keys().collect::<Vec<_>>(),
            library.catalogs["ru"].keys().collect::<Vec<_>>(),
            "Every bundled article needs its own Russian translation"
        );
        assert_eq!(
            library
                .articles("en")
                .iter()
                .map(|a| &a.id)
                .collect::<Vec<_>>(),
            library
                .articles("ru")
                .iter()
                .map(|a| &a.id)
                .collect::<Vec<_>>()
        );
    }
}
