use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use crate::llms::llm_client::LlmClient;
use crate::settings::Settings;
use crate::util;
use crate::{prompt::format_prompt, settings::Language};
use anyhow::Result;

use tokio::task::JoinSet;
use tokio::try_join;

use tera::{Context, Tera};

#[derive(Debug, Clone)]
pub(crate) struct SummarizationClient {
    client: Arc<dyn LlmClient>,

    file_ignore: Vec<String>,
    prompt_file_diff: String,
    prompt_conventional_commit_prefix: String,
    prompt_commit_summary: String,
    prompt_commit_title: String,
    prompt_translation: String,
    output_conventional_commit: bool,
    output_conventional_commit_prefix_format: String,
    output_lang: Language,
    output_show_per_file_summary: bool,
}

impl SummarizationClient {
    pub(crate) fn new(settings: Settings, client: Box<dyn LlmClient>) -> Result<Self> {
        let prompt_settings = settings.prompt.unwrap_or_default();

        let prompt_file_diff = prompt_settings.file_diff.unwrap_or_default();
        let prompt_conventional_commit_prefix = prompt_settings
            .conventional_commit_prefix
            .unwrap_or_default();
        let prompt_commit_summary = prompt_settings.commit_summary.unwrap_or_default();
        let prompt_commit_title = prompt_settings.commit_title.unwrap_or_default();
        let prompt_translation = prompt_settings.translation.unwrap_or_default();

        let output_settings = settings.output.unwrap_or_default();
        let output_conventional_commit = output_settings.conventional_commit.unwrap_or(true);
        let output_conventional_commit_prefix_format = output_settings
            .conventional_commit_prefix_format
            .unwrap_or_default();
        let output_lang =
            Language::from_str(&output_settings.lang.unwrap_or_default()).unwrap_or_default();
        let output_show_per_file_summary = output_settings.show_per_file_summary.unwrap_or(false);
        let file_ignore = settings.file_ignore.unwrap_or_default();
        Ok(Self {
            client: client.into(),
            file_ignore,
            prompt_file_diff,
            prompt_conventional_commit_prefix,
            prompt_commit_summary,
            prompt_commit_title,
            prompt_translation,
            output_lang,
            output_show_per_file_summary,
            output_conventional_commit,
            output_conventional_commit_prefix_format,
        })
    }

    pub(crate) async fn get_commit_message(&self, file_diffs: Vec<&str>, commit_message: &str) -> Result<String> {
        let mut set = JoinSet::new();

        for file_diff in file_diffs {
            let file_diff = file_diff.to_owned();
            let cloned_self = self.clone();
            let commit_message = commit_message.to_string();
            set.spawn(async move { cloned_self.process_file_diff(&file_diff, &commit_message).await });
        }

        let mut summary_for_file: HashMap<String, String> = HashMap::with_capacity(set.len());
        while let Some(res) = set.join_next().await {
            if let Some((k, v)) = res.unwrap() {
                summary_for_file.insert(k, v);
            }
        }

        let summary_points = &summary_for_file
            .iter()
            .map(|(file_name, completion)| format!("[{file_name}]\n{completion}"))
            .collect::<Vec<String>>()
            .join("\n");

        let mut message = String::with_capacity(1024);

        let (title, completion, conventional_commit_prefix) = try_join!(
            self.commit_title(summary_points, commit_message),
            self.commit_summary(summary_points, commit_message),
            self.conventional_commit_prefix(summary_points)
        )?;

        message.push_str(&format!("{title}\n\n{completion}\n\n"));

        if self.output_show_per_file_summary {
            for (file_name, completion) in &summary_for_file {
                if !completion.is_empty() {
                    message.push_str(&format!("[{file_name}]\n{completion}\n"));
                }
            }
        }

        // split message into lines and uniquefy lines
        let mut lines = message.lines().collect::<Vec<&str>>();
        lines.dedup();
        let message = lines.join("\n");

        let mut message = self.commit_translate(&message).await?;
        if !conventional_commit_prefix.is_empty() {
            let mut ctx = Context::new();
            ctx.insert("prefix", conventional_commit_prefix.as_str());
            let formated_prefix =
                Tera::one_off(&self.output_conventional_commit_prefix_format, &ctx, false)?;
            message.insert_str(0, formated_prefix.as_str());
        }

        Ok(message)
    }

    /// Splits the contents of a git diff by file.
    ///
    /// The file path is the first string in the returned tuple, and the
    /// file content is the second string in the returned tuple.
    ///
    /// The function assumes that the file_diff input is well-formed
    /// according to the Diff format described in the Git documentation:
    /// https://git-scm.com/docs/git-diff
    async fn process_file_diff(&self, file_diff: &str, commit_message: &str) -> Option<(String, String)> {
        if let Some(file_name) = util::get_file_name_from_diff(file_diff) {
            if self
                .file_ignore
                .iter()
                .any(|ignore| file_name.contains(ignore))
            {
                warn!("skipping {file_name} due to file_ignore setting");

                return None;
            }
            let completion = self.diff_summary(file_name, file_diff, commit_message).await;
            Some((
                file_name.to_string(),
                completion.unwrap_or_else(|_| "".to_string()),
            ))
        } else {
            None
        }
    }

    async fn diff_summary(&self, file_name: &str, file_diff: &str, commit_message: &str) -> Result<String> {
        debug!("summarizing file: {}", file_name);
        debug!("commit_message: {}", commit_message);

        let prompt = format_prompt(
            &self.prompt_file_diff,
            HashMap::from([("file_diff", file_diff), ("commit_message", commit_message)]),
        )?;
        
        debug!("diff_summary prompt: {}", prompt);

        self.client.completions(&prompt).await
    }

    // TODO use option type and enum here
    pub(crate) async fn conventional_commit_prefix(&self, summary_points: &str) -> Result<String> {
        if !self.output_conventional_commit {
            return Ok("".to_string());
        }
        let prompt = format_prompt(
            &self.prompt_conventional_commit_prefix,
            HashMap::from([("summary_points", summary_points)]),
        )?;

        let completion = self.client.completions(&prompt).await?;
        match completion.to_ascii_lowercase().trim() {
            "build" | "chore" | "ci" | "docs" | "feat" | "fix" | "perf" | "refactor" | "style"
            | "test" => Ok(completion.to_string()),
            _ => Ok("".to_string()),
        }
    }

    pub(crate) async fn commit_summary(&self, summary_points: &str, commit_message: &str) -> Result<String> {
        debug!("commit_message: {}", commit_message);
        let prompt = format_prompt(
            &self.prompt_commit_summary,
            HashMap::from([("summary_points", summary_points), ("commit_message", commit_message)]),
        )?;

        debug!("commit_summary prompt: {}", prompt);

        self.client.completions(&prompt).await
    }

    pub(crate) async fn commit_title(&self, summary_points: &str, commit_message: &str) -> Result<String> {
        debug!("commit_message: {}", commit_message);
        let prompt = format_prompt(
            &self.prompt_commit_title,
            HashMap::from([("summary_points", summary_points), ("commit_message", commit_message)]),
        )?;

        debug!("commit_title prompt: {}", prompt);
        
        self.client.completions(&prompt).await
    }

    pub(crate) async fn commit_translate(&self, commit_message: &str) -> Result<String> {
        if let Language::En = self.output_lang {
            return Ok(commit_message.to_string());
        }
        let prompt = format_prompt(
            &self.prompt_translation,
            HashMap::from([
                ("commit_message", commit_message),
                ("output_language", &self.output_lang.to_string()),
            ]),
        )?;
        self.client.completions(&prompt).await
    }
}
