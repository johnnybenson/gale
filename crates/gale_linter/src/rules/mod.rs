pub mod annotation_no_unknown;
pub mod at_rule_no_unknown;
pub mod block_no_empty;
pub mod color_no_invalid_hex;
pub mod comment_no_empty;
pub mod custom_property_no_missing_var_function;
pub mod declaration_block_no_duplicate_custom_properties;
pub mod declaration_block_no_duplicate_properties;
pub mod declaration_block_no_shorthand_property_overrides;
pub mod font_family_no_duplicate_names;
pub mod font_family_no_missing_generic_family_keyword;
pub mod function_calc_no_unspaced_operator;
pub mod keyframe_block_no_duplicate_selectors;
pub mod media_feature_name_no_unknown;
pub mod media_query_no_invalid;
pub mod keyframe_declaration_no_important;
pub mod no_descending_specificity;
pub mod no_duplicate_at_import_rules;
pub mod no_duplicate_selectors;
pub mod no_empty_source;
pub mod no_invalid_double_slash_comments;
pub mod no_invalid_position_declaration;
pub mod no_invalid_position_at_import_rule;
pub mod no_irregular_whitespace;
pub mod property_no_unknown;
pub mod selector_pseudo_class_no_unknown;
pub mod selector_type_no_unknown;
pub mod selector_pseudo_element_no_unknown;
pub mod string_no_newline;
pub mod unit_no_unknown;

use crate::registry::RuleRegistry;

/// Register all built-in rules in the given registry.
pub fn register_all(registry: &mut RuleRegistry) {
    registry.register(Box::new(annotation_no_unknown::AnnotationNoUnknown));
    registry.register(Box::new(at_rule_no_unknown::AtRuleNoUnknown));
    registry.register(Box::new(block_no_empty::BlockNoEmpty));
    registry.register(Box::new(color_no_invalid_hex::ColorNoInvalidHex));
    registry.register(Box::new(comment_no_empty::CommentNoEmpty));
    registry.register(Box::new(custom_property_no_missing_var_function::CustomPropertyNoMissingVarFunction));
    registry.register(Box::new(declaration_block_no_duplicate_custom_properties::DeclarationBlockNoDuplicateCustomProperties));
    registry.register(Box::new(declaration_block_no_duplicate_properties::DeclarationBlockNoDuplicateProperties));
    registry.register(Box::new(declaration_block_no_shorthand_property_overrides::DeclarationBlockNoShorthandPropertyOverrides));
    registry.register(Box::new(font_family_no_duplicate_names::FontFamilyNoDuplicateNames));
    registry.register(Box::new(font_family_no_missing_generic_family_keyword::FontFamilyNoMissingGenericFamilyKeyword));
    registry.register(Box::new(function_calc_no_unspaced_operator::FunctionCalcNoUnspacedOperator));
    registry.register(Box::new(keyframe_block_no_duplicate_selectors::KeyframeBlockNoDuplicateSelectors));
    registry.register(Box::new(keyframe_declaration_no_important::KeyframeDeclarationNoImportant));
    registry.register(Box::new(media_feature_name_no_unknown::MediaFeatureNameNoUnknown));
    registry.register(Box::new(media_query_no_invalid::MediaQueryNoInvalid));
    // TODO: optimize before enabling — O(n²) on large files
    // registry.register(Box::new(no_descending_specificity::NoDescendingSpecificity));
    registry.register(Box::new(no_duplicate_at_import_rules::NoDuplicateAtImportRules));
    registry.register(Box::new(no_duplicate_selectors::NoDuplicateSelectors));
    registry.register(Box::new(no_empty_source::NoEmptySource));
    registry.register(Box::new(no_invalid_double_slash_comments::NoInvalidDoubleSlashComments));
    registry.register(Box::new(no_invalid_position_at_import_rule::NoInvalidPositionAtImportRule));
    registry.register(Box::new(no_invalid_position_declaration::NoInvalidPositionDeclaration));
    registry.register(Box::new(no_irregular_whitespace::NoIrregularWhitespace));
    registry.register(Box::new(property_no_unknown::PropertyNoUnknown));
    registry.register(Box::new(selector_pseudo_class_no_unknown::SelectorPseudoClassNoUnknown));
    registry.register(Box::new(selector_pseudo_element_no_unknown::SelectorPseudoElementNoUnknown));
    registry.register(Box::new(selector_type_no_unknown::SelectorTypeNoUnknown));
    registry.register(Box::new(string_no_newline::StringNoNewline));
    registry.register(Box::new(unit_no_unknown::UnitNoUnknown));
}
