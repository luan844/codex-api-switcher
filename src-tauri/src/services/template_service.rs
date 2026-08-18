use std::fs;

use crate::errors::app_error::AppResult;
use crate::models::template::{SaveTemplateInput, TemplateKind, TemplateSnapshot};
use crate::state::app_state::AppPaths;
use crate::utils::fs_utils::ensure_directory;

pub struct TemplateService<'a> {
    paths: &'a AppPaths,
}

impl<'a> TemplateService<'a> {
    pub fn new(paths: &'a AppPaths) -> Self {
        Self { paths }
    }

    pub fn load_snapshot(&self) -> AppResult<TemplateSnapshot> {
        Ok(TemplateSnapshot {
            open_ai: self.load_openai_template()?,
            api_key: self.load_apikey_template()?,
        })
    }

    pub fn save_template(&self, payload: &SaveTemplateInput) -> AppResult<()> {
        ensure_directory(&self.paths.templates_root)?;
        let content = normalize_line_endings(&payload.content);
        match payload.kind {
            TemplateKind::OpenAi => fs::write(&self.paths.openai_template_path, content)?,
            TemplateKind::ApiKey => fs::write(&self.paths.apikey_template_path, content)?,
        }
        Ok(())
    }

    pub fn reset_template(&self, kind: TemplateKind) -> AppResult<()> {
        let content = match kind {
            TemplateKind::OpenAi => Self::default_openai_template(),
            TemplateKind::ApiKey => Self::default_apikey_template(),
        };
        self.save_template(&SaveTemplateInput {
            kind,
            content: content.to_string(),
        })
    }

    pub fn load_openai_template(&self) -> AppResult<String> {
        self.load_or_create(
            &self.paths.openai_template_path,
            Self::default_openai_template(),
        )
    }

    pub fn load_apikey_template(&self) -> AppResult<String> {
        let current = self.load_or_create(
            &self.paths.apikey_template_path,
            Self::default_apikey_template(),
        )?;
        let normalized = normalize_line_endings(&current);
        let legacy = normalize_line_endings(Self::legacy_default_apikey_template());
        let previous = normalize_line_endings(Self::quoted_provider_name_apikey_template());

        if normalized == legacy || normalized == previous {
            let upgraded = normalize_line_endings(Self::default_apikey_template());
            fs::write(&self.paths.apikey_template_path, &upgraded)?;
            return Ok(upgraded);
        }

        Ok(normalized)
    }

    pub fn default_openai_template() -> &'static str {
        "model = \"gpt-5.4\"\nmodel_reasoning_effort = \"xhigh\"\n"
    }

    pub fn default_apikey_template() -> &'static str {
        "model_provider = {provider_name}\nmodel = \"gpt-5.4\"\nmodel_reasoning_effort = \"xhigh\"\n\ndisable_response_storage = true\n\n[model_providers.{provider_key}]\nname = {provider_name}\nbase_url = {base_url}\nwire_api = \"responses\"\nrequires_openai_auth = true\n"
    }

    fn quoted_provider_name_apikey_template() -> &'static str {
        "model_provider = {provider_name}\nmodel = \"gpt-5.4\"\nmodel_reasoning_effort = \"xhigh\"\n\ndisable_response_storage = true\n\n[model_providers.{provider_name}]\nname = {provider_name}\nbase_url = {base_url}\nwire_api = \"responses\"\nrequires_openai_auth = true\n"
    }

    fn legacy_default_apikey_template() -> &'static str {
        "model_provider = \"right\"\nmodel = \"gpt-5.4\"\nmodel_reasoning_effort = \"xhigh\"\n\ndisable_response_storage = true\n\n[model_providers.right]\nname = \"right\"\nbase_url = {base_url}\nwire_api = \"responses\"\nrequires_openai_auth = true\n"
    }

    fn load_or_create(&self, path: &std::path::Path, default: &str) -> AppResult<String> {
        ensure_directory(&self.paths.templates_root)?;
        if !path.exists() {
            let normalized = normalize_line_endings(default);
            fs::write(path, &normalized)?;
            return Ok(normalized);
        }

        Ok(normalize_line_endings(&fs::read_to_string(path)?))
    }
}

fn normalize_line_endings(content: &str) -> String {
    content.replace("\r\n", "\n").replace('\r', "\n")
}
