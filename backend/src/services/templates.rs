//! Template rendering with MiniJinja.
//!
//! * `{{ variable }}` syntax; undefined variables are errors (strict mode),
//!   so a missing value fails the request instead of sending a broken email.
//! * HTML bodies are auto-escaped; the result is sanitized again afterwards.
//! * Subjects must render to a single line (header-injection guard).
//! * Rendering runs with a fuel budget so a template cannot loop forever.

use std::collections::BTreeSet;

use minijinja::{AutoEscape, Environment, UndefinedBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{
    error::{ApiResult, AppError},
    services::sanitize,
    validation,
};

pub const MAX_TEMPLATE_BYTES: usize = 512 * 1024;
const FUEL: u64 = 200_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableSpec {
    pub name: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Example used for previews and test sends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct TemplateSource<'a> {
    pub subject: &'a str,
    pub html: Option<&'a str>,
    pub text: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Rendered {
    pub subject: String,
    pub html: Option<String>,
    pub text: Option<String>,
}

fn environment<'a>() -> Environment<'a> {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env.set_fuel(Some(FUEL));
    env.set_auto_escape_callback(|name| if name == "html" { AutoEscape::Html } else { AutoEscape::None });
    env
}

fn template_error(part: &str, e: minijinja::Error) -> AppError {
    let mut msg = format!("{part}: {}", e.kind());
    if let Some(detail) = e.detail() {
        msg.push_str(&format!(" ({detail})"));
    }
    if let Some(line) = e.line() {
        msg.push_str(&format!(" at line {line}"));
    }
    AppError::new(axum::http::StatusCode::UNPROCESSABLE_ENTITY, "template_error", msg)
}

/// Validate syntax and size without rendering. Returns variable names used.
pub fn validate(src: &TemplateSource) -> ApiResult<Vec<String>> {
    let total = src.subject.len() + src.html.map_or(0, str::len) + src.text.map_or(0, str::len);
    if total > MAX_TEMPLATE_BYTES {
        return Err(AppError::validation(format!("Template is larger than {} KB", MAX_TEMPLATE_BYTES / 1024)));
    }
    if src.html.is_none() && src.text.is_none() {
        return Err(AppError::validation("A template needs an HTML body, a text body, or both"));
    }
    let env = environment();
    let mut names = BTreeSet::new();
    for (part, name, source) in
        [("subject", "subject", Some(src.subject)), ("html_body", "html", src.html), ("text_body", "text", src.text)]
    {
        if let Some(source) = source {
            let tmpl = env.template_from_named_str(name, source).map_err(|e| template_error(part, e))?;
            names.extend(tmpl.undeclared_variables(false));
        }
    }
    Ok(names.into_iter().collect())
}

/// Merge caller variables with declared defaults and check required ones.
pub fn resolve_variables(specs: &[VariableSpec], provided: &Map<String, Value>, use_samples: bool) -> ApiResult<Value> {
    let mut vars = provided.clone();
    let mut missing = Vec::new();
    for spec in specs {
        if vars.contains_key(&spec.name) {
            continue;
        }
        if let Some(d) = &spec.default {
            vars.insert(spec.name.clone(), d.clone());
        } else if use_samples && spec.sample.is_some() {
            vars.insert(spec.name.clone(), spec.sample.clone().unwrap());
        } else if spec.required {
            missing.push(spec.name.clone());
        }
    }
    if !missing.is_empty() {
        return Err(AppError::new(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "missing_template_variables",
            format!("Missing required template variables: {}", missing.join(", ")),
        )
        .with_details(serde_json::json!({ "missing": missing })));
    }
    Ok(Value::Object(vars))
}

pub fn render(src: &TemplateSource, vars: &Value) -> ApiResult<Rendered> {
    let env = environment();
    let render_one = |part: &str, name: &str, source: &str| -> ApiResult<String> {
        let tmpl = env.template_from_named_str(name, source).map_err(|e| template_error(part, e))?;
        tmpl.render(vars).map_err(|e| template_error(part, e))
    };

    let subject = render_one("subject", "subject", src.subject)?.trim().to_string();
    if subject.is_empty() {
        return Err(AppError::validation("Rendered subject is empty"));
    }
    validation::header_safe("Rendered subject", &subject, 998)?;

    let html = match src.html {
        Some(h) => Some(sanitize::outbound(&render_one("html_body", "html", h)?)),
        None => None,
    };
    let text = match src.text {
        Some(t) => Some(render_one("text_body", "text", t)?),
        None => None,
    };
    Ok(Rendered { subject, html, text })
}

pub fn parse_specs(value: &Value) -> Vec<VariableSpec> {
    serde_json::from_value(value.clone()).unwrap_or_default()
}

pub fn validate_specs(specs: &[VariableSpec]) -> ApiResult<()> {
    let mut seen = BTreeSet::new();
    for s in specs {
        if s.name.is_empty() || s.name.len() > 64 || !s.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(AppError::validation(format!("Invalid variable name {:?} (use letters, digits, '_')", s.name)));
        }
        if !seen.insert(&s.name) {
            return Err(AppError::validation(format!("Variable {:?} is declared twice", s.name)));
        }
    }
    if specs.len() > 100 {
        return Err(AppError::validation("At most 100 variables may be declared"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn src<'a>(subject: &'a str, html: &'a str) -> TemplateSource<'a> {
        TemplateSource { subject, html: Some(html), text: Some("Hi {{ name }}") }
    }

    #[test]
    fn renders_with_escaping_and_sanitizing() {
        let r = render(
            &src("Welcome {{ name }}", "<p>Hello {{ name }}</p><a href=\"{{ url }}\">go</a>"),
            &json!({"name": "<b>Ann</b>", "url": "javascript:alert(1)"}),
        )
        .unwrap();
        assert_eq!(r.subject, "Welcome <b>Ann</b>");
        let html = r.html.unwrap();
        assert!(html.contains("&lt;b&gt;Ann&lt;/b&gt;"));
        assert!(!html.contains("javascript:"));
        assert_eq!(r.text.unwrap(), "Hi <b>Ann</b>");
    }

    #[test]
    fn strict_undefined_and_subject_injection() {
        assert!(render(&src("Hi {{ missing }}", "x"), &json!({})).is_err());
        let err = render(&src("Hi {{ name }}", "x"), &json!({"name": "a\r\nBcc: x@y.z"}));
        assert!(err.is_err());
    }

    #[test]
    fn fuel_stops_runaway_loops() {
        let r = render(
            &src("x", "{% for i in range(100000) %}{% for j in range(100000) %}.{% endfor %}{% endfor %}"),
            &json!({}),
        );
        assert!(r.is_err());
    }

    #[test]
    fn variables() {
        let specs = vec![
            VariableSpec {
                name: "name".into(),
                required: true,
                default: None,
                description: None,
                sample: Some(json!("Ann")),
            },
            VariableSpec {
                name: "plan".into(),
                required: false,
                default: Some(json!("Free")),
                description: None,
                sample: None,
            },
        ];
        assert!(resolve_variables(&specs, &Map::new(), false).is_err());
        let v = resolve_variables(&specs, &Map::new(), true).unwrap();
        assert_eq!(v["name"], "Ann");
        assert_eq!(v["plan"], "Free");
        assert_eq!(validate(&src("Hi {{ name }}", "{{ plan }}")).unwrap(), vec!["name", "plan"]);
    }
}
