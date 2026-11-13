// -----------------------------------------------------------------------------
//+ Authors: 	Ran# <ran.hash@proton.me>
//+ Created: 	2026/11/12 10:31:30.046143
//+ Revised: 	2026/11/12 10:31:30.046143
// -----------------------------------------------------------------------------

//! Session options an agent offers (model, reasoning effort, mode), read from ACP.
//!
//! Current adapters describe them as `configOptions` and change them with
//! `session/set_config_option`; older ones only list `models` and use `session/set_model`.
//! Both are presented the same way.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Option categories shown as pickers, in display order.
pub const SHOWN_CATEGORIES: [&str; 3] = ["model", "thought_level", "mode"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigChoice {
    pub value: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigOption {
    pub id: String,
    pub name: String,
    /// ACP category such as `model`, `thought_level`, or `mode`; `None` for agent-specific options.
    pub category: Option<String>,
    pub current: String,
    pub choices: Vec<ConfigChoice>,
    /// Set through the older `session/set_model` request.
    pub legacy_model: bool,
}

impl ConfigOption {
    pub fn current_name(&self) -> &str {
        self.choices
            .iter()
            .find(|choice| choice.value == self.current)
            .map_or(self.current.as_str(), |choice| choice.name.as_str())
    }

    pub fn offers(&self, value: &str) -> bool {
        self.choices.iter().any(|choice| choice.value == value)
    }

    /// Shown as a picker in the session header.
    pub fn shown(&self) -> bool {
        self.category
            .as_deref()
            .is_some_and(|category| SHOWN_CATEGORIES.contains(&category))
    }
}

/// The options in a `session/new` (or `session/set_config_option`) result.
pub fn from_session(result: &Value) -> Vec<ConfigOption> {
    let mut options = from_config_options(&result["configOptions"]);
    let has_model = options
        .iter()
        .any(|option| option.category.as_deref() == Some("model"));
    if !has_model && let Some(model) = from_models(&result["models"]) {
        options.insert(0, model);
    }
    options
}

/// Select-type entries of a `configOptions` array; other kinds are not shown yet.
pub fn from_config_options(value: &Value) -> Vec<ConfigOption> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter(|option| option["type"].as_str().is_none_or(|kind| kind == "select"))
        .filter_map(|option| {
            let id = option["id"].as_str().filter(|id| !id.is_empty())?;
            let mut choices = Vec::new();
            collect_choices(&option["options"], &mut choices);
            let current = option["currentValue"].as_str()?.to_owned();
            (!choices.is_empty()).then(|| ConfigOption {
                id: id.to_owned(),
                name: option["name"].as_str().unwrap_or(id).to_owned(),
                category: option["category"].as_str().map(str::to_owned),
                current,
                choices,
                legacy_model: false,
            })
        })
        .collect()
}

/// Choices may be flat or grouped (`{group, name, options: [...]}`).
fn collect_choices(value: &Value, choices: &mut Vec<ConfigChoice>) {
    for entry in value.as_array().into_iter().flatten() {
        if let Some(value) = entry["value"].as_str() {
            choices.push(ConfigChoice {
                value: value.to_owned(),
                name: entry["name"].as_str().unwrap_or(value).to_owned(),
                description: entry["description"].as_str().map(str::to_owned),
            });
        } else if entry["options"].is_array() {
            collect_choices(&entry["options"], choices);
        }
    }
}

/// The older `models` block: `{currentModelId, availableModels: [{modelId, name}]}`.
fn from_models(value: &Value) -> Option<ConfigOption> {
    let current = value["currentModelId"].as_str()?.to_owned();
    let choices: Vec<ConfigChoice> = value["availableModels"]
        .as_array()?
        .iter()
        .filter_map(|model| {
            let id = model["modelId"].as_str()?;
            Some(ConfigChoice {
                value: id.to_owned(),
                name: model["name"].as_str().unwrap_or(id).to_owned(),
                description: model["description"].as_str().map(str::to_owned),
            })
        })
        .collect();
    (!choices.is_empty()).then(|| ConfigOption {
        id: "model".into(),
        name: "Model".into(),
        category: Some("model".into()),
        current,
        choices,
        legacy_model: true,
    })
}

/// The request that sets `option` to `value` in `session`.
pub fn set_request(id: u64, session: &str, option: &ConfigOption, value: &str) -> Value {
    if option.legacy_model {
        super::protocol::request(
            id,
            "session/set_model",
            json!({"sessionId": session, "modelId": value}),
        )
    } else {
        super::protocol::request(
            id,
            "session/set_config_option",
            json!({"sessionId": session, "configId": option.id, "value": value}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claude_like() -> Value {
        json!({
            "sessionId": "s",
            "configOptions": [
                {"id": "mode", "name": "Mode", "category": "mode", "type": "select", "currentValue": "default",
                 "options": [{"value": "default", "name": "Default"}, {"value": "plan", "name": "Plan"}]},
                {"id": "model", "name": "Model", "category": "model", "type": "select", "currentValue": "opus",
                 "options": [{"group": "latest", "name": "Latest",
                              "options": [{"value": "opus", "name": "Opus"}, {"value": "sonnet", "name": "Sonnet"}]}]},
                {"id": "fast", "name": "Fast", "category": "model_config", "type": "select", "currentValue": "off",
                 "options": [{"value": "on", "name": "On"}, {"value": "off", "name": "Off"}]},
                {"id": "notes", "name": "Notes", "type": "text", "currentValue": ""},
                {"id": "", "type": "select", "currentValue": "x", "options": [{"value": "x"}]}
            ]
        })
    }

    #[test]
    fn unit_config_options_are_read_with_grouped_choices() {
        let options = from_session(&claude_like());
        let ids: Vec<&str> = options.iter().map(|option| option.id.as_str()).collect();
        assert_eq!(ids, ["mode", "model", "fast"]);
        let model = &options[1];
        assert_eq!(model.current_name(), "Opus");
        assert!(model.offers("sonnet") && !model.offers("gpt"));
        assert!(model.shown() && options[0].shown() && !options[2].shown());
        assert!(!model.legacy_model);
    }

    #[test]
    fn unit_older_model_lists_become_a_model_option() {
        let result = json!({
            "sessionId": "s",
            "models": {"currentModelId": "gpt-a", "availableModels": [
                {"modelId": "gpt-a", "name": "GPT A"}, {"modelId": "gpt-b"}]},
            "configOptions": [{"id": "effort", "category": "thought_level", "type": "select",
                               "currentValue": "low", "options": [{"value": "low"}, {"value": "high"}]}]
        });
        let options = from_session(&result);
        assert_eq!(options[0].id, "model");
        assert!(options[0].legacy_model);
        assert_eq!(options[0].choices[1].name, "gpt-b");
        assert_eq!(options[1].current_name(), "low");
    }

    #[test]
    fn unit_models_are_ignored_when_config_options_already_list_one() {
        let mut result = claude_like();
        result["models"] = json!({"currentModelId": "x", "availableModels": [{"modelId": "x"}]});
        let options = from_session(&result);
        assert_eq!(
            options
                .iter()
                .filter(|option| option.category.as_deref() == Some("model"))
                .count(),
            1
        );
        assert!(from_session(&json!({"sessionId": "s"})).is_empty());
    }

    #[test]
    fn unit_set_requests_use_the_method_the_agent_understands() {
        let options = from_session(&claude_like());
        let request = set_request(7, "s", &options[1], "sonnet");
        assert_eq!(request["method"], "session/set_config_option");
        assert_eq!(
            request["params"],
            json!({"sessionId": "s", "configId": "model", "value": "sonnet"})
        );
        let legacy = from_session(
            &json!({"models": {"currentModelId": "a", "availableModels": [{"modelId": "a"}]}}),
        );
        let request = set_request(8, "s", &legacy[0], "a");
        assert_eq!(request["method"], "session/set_model");
        assert_eq!(request["params"], json!({"sessionId": "s", "modelId": "a"}));
    }
}
