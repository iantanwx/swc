use swc_common::Span;
use swc_css_ast::*;
use swc_css_visit::{Visit, VisitWith};

use crate::rule::{visitor_rule, LintRule, LintRuleContext};

pub fn keyframe_declaration_no_important(ctx: LintRuleContext<()>) -> Box<dyn LintRule> {
    visitor_rule(KeyframeDeclarationNoImportant {
        ctx,
        keyframe_rules: vec![],
    })
}

const MESSAGE: &str = "Unexpected '!important'.";

#[derive(Debug, Default)]
struct KeyframeDeclarationNoImportant {
    ctx: LintRuleContext<()>,

    // rule internal
    keyframe_rules: Vec<Span>,
}

impl Visit for KeyframeDeclarationNoImportant {
    fn visit_keyframes_rule(&mut self, keyframes_rule: &KeyframesRule) {
        self.keyframe_rules.push(keyframes_rule.span);

        keyframes_rule.visit_children_with(self);

        self.keyframe_rules.pop();
    }

    fn visit_important_flag(&mut self, important_flag: &ImportantFlag) {
        match self.keyframe_rules.last() {
            Some(span) if span.contains(important_flag.span) => {
                self.ctx.report(important_flag, MESSAGE);
            }
            _ => {}
        }

        important_flag.visit_children_with(self);
    }
}
