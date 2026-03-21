use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::data::is_known_at_rule;
use crate::rule::{Rule, RuleContext};

pub struct AtRuleNoUnknown;

impl Rule for AtRuleNoUnknown {
    fn name(&self) -> &'static str {
        "at-rule-no-unknown"
    }

    fn description(&self) -> &'static str {
        "Disallow unknown at-rules"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::AtRule(at) = node else {
            return vec![];
        };
        // Skip vendor-prefixed
        if at.name.starts_with('-') {
            return vec![];
        }
        if !is_known_at_rule(&at.name) {
            vec![
                Diagnostic::new(self.name(), format!("Unexpected unknown at-rule \"@{}\"", at.name))
                    .severity(self.default_severity())
                    .span(Span::new(at.span.offset, at.span.length)),
            ]
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{AtRule, CssNode, Span, Syntax};

    fn ctx() -> RuleContext<'static> {
        RuleContext { file_path: "t.css", source: "", syntax: Syntax::Css }
    }

    fn at(name: &str) -> CssNode {
        CssNode::AtRule(AtRule {
            name: name.to_string(),
            params: String::new(),
            span: Span::new(0, 0),
            children: vec![],
        })
    }

    #[test]
    fn reports_unknown() {
        let d = AtRuleNoUnknown.check(&at("tailwind"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("@tailwind"));
    }

    #[test]
    fn allows_known() {
        assert!(AtRuleNoUnknown.check(&at("media"), &ctx()).is_empty());
        assert!(AtRuleNoUnknown.check(&at("keyframes"), &ctx()).is_empty());
    }

    #[test]
    fn skips_vendor_prefixed() {
        assert!(AtRuleNoUnknown.check(&at("-webkit-keyframes"), &ctx()).is_empty());
    }
}
