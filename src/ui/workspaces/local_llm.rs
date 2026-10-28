//! エージェント画面のローカル LLM の設定と疎通の確認（§15.8.4）。
use super::tabs::{label, State};
use crate::ui::{
    i18n::{self, Locale},
    workbench::Workbench,
};
use egui::Ui;

pub(super) fn local_ui(
    ui: &mut Ui,
    state: &mut State,
    bench: &mut Workbench,
    locale: Locale,
    now: f64,
    host: &mut dyn crate::ui::app::Host,
) {
    if !host.mcp_available() {
        ui.label(label(locale, "stage.local_web"));
        return;
    }
    if !state.local_loaded {
        let agents = &bench.project().settings().agents;
        state.local_enabled = agents.local_llm.enabled;
        state.local_url = agents.local_llm.url.clone();
        state.local_model = agents.local_llm.model.clone();
        state.custom_roles = agents.custom_roles.join(", ");
        state.local_loaded = true;
    }
    ui.label(label(locale, "stage.local_caution"));
    ui.checkbox(&mut state.local_enabled, label(locale, "stage.enabled"));
    ui.text_edit_singleline(&mut state.local_url);
    ui.text_edit_singleline(&mut state.local_model);
    ui.label(label(locale, "stage.custom_roles"));
    ui.text_edit_singleline(&mut state.custom_roles);
    if ui.button(label(locale, "stage.confirm")).clicked() {
        let config = crate::core::model::project::LocalLlm {
            enabled: state.local_enabled,
            url: state.local_url.clone(),
            model: state.local_model.clone(),
            ..Default::default()
        };
        match crate::core::features::local_llm::configure(
            bench.project().text("agents/mcp.json"),
            &config,
        ) {
            Ok(mcp) => {
                // project.toml は無損失編集で書き戻し、壊れた TOML は上書きしない（§4.5）
                let roles: Vec<String> = state
                    .custom_roles
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect();
                if let Some(edited) = crate::core::syntax::project_toml::edit(
                    bench
                        .project()
                        .text(crate::core::model::project::PROJECT_FILE),
                    |document| {
                        use toml_edit::{Item, Table, Value};
                        let root = document.as_table_mut();
                        if root.get("agents").is_none() {
                            root.insert("agents", Item::Table(Table::new()));
                        }
                        let Some(agents) = root.get_mut("agents").and_then(Item::as_table_like_mut)
                        else {
                            return;
                        };
                        let mut array = toml_edit::Array::new();
                        for role in &roles {
                            array.push(role.as_str());
                        }
                        crate::core::syntax::project_toml::set_value(
                            agents,
                            "custom_roles",
                            Value::Array(array),
                        );
                        if agents.get("local_llm").is_none() {
                            agents.insert(
                                "local_llm",
                                Item::Value(Value::InlineTable(Default::default())),
                            );
                        }
                        let Some(local) = agents
                            .get_mut("local_llm")
                            .and_then(Item::as_table_like_mut)
                        else {
                            return;
                        };
                        crate::core::syntax::project_toml::set_value(
                            local,
                            "enabled",
                            toml_edit::Value::from(config.enabled),
                        );
                        crate::core::syntax::project_toml::set_string(local, "url", &config.url);
                        crate::core::syntax::project_toml::set_string(
                            local,
                            "model",
                            &config.model,
                        );
                    },
                ) {
                    bench.replace_document("agents/mcp.json", mcp, now);
                    bench.replace_document(crate::core::model::project::PROJECT_FILE, edited, now);
                }
            }
            Err(e) => state.local_result = e.to_owned(),
        }
    }
    if state.local_job.is_none() && ui.button(label(locale, "stage.connection_check")).clicked() {
        if crate::core::features::local_llm::is_loopback(&state.local_url) {
            state.local_job = host.local_probe_start(crate::ui::agents::Job {
                server: crate::ports::mcp::McpServer {
                    name: "novnow-local-llm".to_owned(),
                    command: "__novnow_http_mcp".to_owned(),
                    args: vec![state.local_url.clone(), state.local_model.clone()],
                    env: Vec::new(),
                    timeout_ms: 3000,
                },
                call: crate::ports::mcp::McpCall::Tool {
                    name: "__novnow_probe".to_owned(),
                    arguments: serde_json::Value::Null,
                },
            });
        } else {
            state.local_result = "E064".to_owned();
        }
    }
    for (id, result, _) in host.local_probe_poll() {
        if state.local_job == Some(id) {
            state.local_job = None;
            let diagnostic = match result {
                Ok(reply) => crate::core::diagnostics::Diagnostic::new(
                    crate::core::diagnostics::DiagnosticCode::I057,
                    None,
                    vec![crate::core::diagnostics::Arg::text(reply.model)],
                ),
                Err(error) => crate::core::diagnostics::Diagnostic::new(
                    crate::core::diagnostics::DiagnosticCode::E064,
                    None,
                    vec![crate::core::diagnostics::Arg::text(error.log_text())],
                ),
            };
            state.local_result = i18n::diagnostic(locale, &diagnostic);
        }
    }
    if state.local_job.is_some() {
        ui.spinner();
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
    }
    ui.label(&state.local_result);
}
