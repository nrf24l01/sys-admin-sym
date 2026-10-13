use cloud_provider_sim::{DeviceId, FirstVmGuide, FirstVmStep};

pub struct WikiWindowState {
    pub open: bool,
    pub article: String,
    pub history: Vec<String>,
    pub show_articles: bool,
    pub controller: Option<DeviceId>,
    pub node: Option<DeviceId>,
    pub step: Option<FirstVmStep>,
    pub report: Option<FirstVmGuide>,
    pub anchor: Option<String>,
    pub link_error: Option<String>,
}
impl Default for WikiWindowState {
    fn default() -> Self {
        Self {
            open: false,
            article: "tutorial/first-vm".into(),
            history: Vec::new(),
            show_articles: false,
            controller: None,
            node: None,
            step: None,
            report: None,
            anchor: None,
            link_error: None,
        }
    }
}
impl WikiWindowState {
    pub fn navigate(&mut self, article: &str) {
        if self.article != article {
            self.history
                .push(std::mem::replace(&mut self.article, article.to_owned()));
        }
        self.anchor = None;
        self.step = None;
        self.link_error = None;
    }
}
