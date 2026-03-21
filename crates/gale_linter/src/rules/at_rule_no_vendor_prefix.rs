use gale_css_parser::CssNode;
use gale_diagnostics::{Diagnostic, Severity, Span};

use crate::rule::{Rule, RuleContext};

/// Reports vendor-prefixed at-rules (e.g. `@-webkit-keyframes`).
///
/// Equivalent to Stylelint's `at-rule-no-vendor-prefix` rule.
pub struct AtRuleNoVendorPrefix;

impl Rule for AtRuleNoVendorPrefix {
    fn name(&self) -> &'static str {
        "at-rule-no-vendor-prefix"
    }

    fn description(&self) -> &'static str {
        "Disallow vendor prefixes for at-rules"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, node: &CssNode, _ctx: &RuleContext) -> Vec<Diagnostic> {
        let CssNode::AtRule(rule) = node else {
            return vec![];
        };
        if rule.name.starts_with('-') {
            vec![
                Diagnostic::new(
                    self.name(),
                    format!("Unexpected vendor-prefixed at-rule \"@{}\"", rule.name),
                )
                .severity(self.default_severity())
                .span(Span::new(rule.span.offset, rule.span.length)),
            ]
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gale_css_parser::{AtRule as CssAtRule, Span as ParserSpan, Syntax};

    fn ctx() -> RuleContext<'static> {
        RuleContext { file_path: "t.css", source: "", syntax: Syntax::Css }
    }

    fn at_rule(name: &str) -> CssNode {
        CssNode::AtRule(CssAtRule {
            name: name.to_string(),
            params: "fade".to_string(),
            span: ParserSpan::new(0, 0),
            children: vec![],
        })
    }

    #[test]
    fn reports_vendor_prefixed_at_rule() {
        let d = AtRuleNoVendorPrefix.check(&at_rule("-webkit-keyframes"), &ctx());
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("-webkit-keyframes"));
    }

    #[test]
    fn allows_standard_at_rule() {
        assert!(AtRuleNoVendorPrefix.check(&at_rule("keyframes"), &ctx()).is_empty());
        assert!(AtRuleNoVendorPrefix.check(&at_rule("media"), &ctx()).is_empty());
    }
}
