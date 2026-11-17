//! The rules' parameters, as set in `[lint.options.<code>]` of `jtk.toml`. Defaults are those of the Checkstyle
//! check or PMD rule the rule follows, under the same names in kebab case (`max`, `allowed-distance`, …).

use serde::Deserialize;

/// The parameters of all rules that have some.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RuleOptions {
    #[serde(rename = "I002")]
    pub illegal_import: IllegalImport,
    #[serde(rename = "B020")]
    pub illegal_catch: IllegalTypes,
    #[serde(rename = "B021")]
    pub illegal_throws: IllegalTypes,
    #[serde(rename = "N013")]
    pub abbreviation_in_name: AbbreviationInName,
    #[serde(rename = "A014")]
    pub declaration_usage_distance: DeclarationUsageDistance,
    #[serde(rename = "A015")]
    pub magic_number: MagicNumber,
    #[serde(rename = "C001")]
    pub cyclomatic_complexity: Max,
    #[serde(rename = "C003")]
    pub boolean_expression_complexity: Max,
    #[serde(rename = "C004")]
    pub java_ncss: JavaNcss,
    #[serde(rename = "C005")]
    pub executable_statement_count: Max,
    #[serde(rename = "C006")]
    pub method_length: Max,
    #[serde(rename = "C007")]
    pub file_length: Max,
    #[serde(rename = "C008")]
    pub lambda_body_length: Max,
    #[serde(rename = "C009")]
    pub anon_inner_length: Max,
    #[serde(rename = "C010")]
    pub parameter_number: Max,
    #[serde(rename = "C011")]
    pub record_component_number: Max,
    #[serde(rename = "C013")]
    pub outer_type_number: Max,
    #[serde(rename = "C014")]
    pub nested_if_depth: Max,
    #[serde(rename = "C015")]
    pub nested_for_depth: Max,
    #[serde(rename = "C016")]
    pub nested_try_depth: Max,
    #[serde(rename = "C020")]
    pub line_length: Max,
    #[serde(rename = "C021")]
    pub throws_count: Max,
    #[serde(rename = "C022")]
    pub cognitive_complexity: CognitiveComplexity,
}

impl Default for RuleOptions {
    /// Checkstyle's defaults, and PMD's for C022. IllegalThrows allows `Exception`, unlike IllegalCatch.
    fn default() -> Self {
        RuleOptions {
            illegal_import: IllegalImport::default(),
            illegal_catch: IllegalTypes::of(&["Exception", "Error", "RuntimeException", "Throwable"]),
            illegal_throws: IllegalTypes::of(&["Error", "RuntimeException", "Throwable"]),
            abbreviation_in_name: AbbreviationInName::default(),
            declaration_usage_distance: DeclarationUsageDistance::default(),
            magic_number: MagicNumber::default(),
            cyclomatic_complexity: Max { max: 10 },
            boolean_expression_complexity: Max { max: 3 },
            java_ncss: JavaNcss::default(),
            executable_statement_count: Max { max: 30 },
            method_length: Max { max: 150 },
            file_length: Max { max: 2000 },
            lambda_body_length: Max { max: 10 },
            anon_inner_length: Max { max: 20 },
            parameter_number: Max { max: 7 },
            record_component_number: Max { max: 8 },
            outer_type_number: Max { max: 1 },
            nested_if_depth: Max { max: 1 },
            nested_for_depth: Max { max: 1 },
            nested_try_depth: Max { max: 1 },
            line_length: Max { max: 80 },
            throws_count: Max { max: 4 },
            cognitive_complexity: CognitiveComplexity::default(),
        }
    }
}

/// The most of something a rule allows.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Max {
    pub max: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct IllegalImport {
    pub illegal_pkgs: Vec<String>,
}

impl Default for IllegalImport {
    fn default() -> Self {
        IllegalImport { illegal_pkgs: strings(&["sun"]) }
    }
}

/// The exception types a rule reports, also when qualified with `java.lang`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct IllegalTypes {
    pub illegal_class_names: Vec<String>,
}

impl IllegalTypes {
    fn of(names: &[&str]) -> IllegalTypes {
        IllegalTypes { illegal_class_names: strings(names) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct AbbreviationInName {
    /// The longest abbreviation allowed, not counting the capital that starts the next word.
    pub allowed_abbreviation_length: u32,
}

impl Default for AbbreviationInName {
    fn default() -> Self {
        AbbreviationInName { allowed_abbreviation_length: 3 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct DeclarationUsageDistance {
    pub allowed_distance: u32,
}

impl Default for DeclarationUsageDistance {
    fn default() -> Self {
        DeclarationUsageDistance { allowed_distance: 3 }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct MagicNumber {
    pub ignore_numbers: Vec<f64>,
}

impl Default for MagicNumber {
    fn default() -> Self {
        MagicNumber { ignore_numbers: vec![-1.0, 0.0, 1.0, 2.0] }
    }
}

/// Checkstyle JavaNCSS's `methodMaximum`, `classMaximum`, `recordMaximum` and `fileMaximum`.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct JavaNcss {
    pub method_maximum: u32,
    pub class_maximum: u32,
    pub record_maximum: u32,
    pub file_maximum: u32,
}

impl Default for JavaNcss {
    fn default() -> Self {
        JavaNcss { method_maximum: 50, class_maximum: 1500, record_maximum: 150, file_maximum: 2000 }
    }
}

/// PMD's `reportLevel`: methods whose complexity reaches it are reported.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct CognitiveComplexity {
    pub report_level: u32,
}

impl Default for CognitiveComplexity {
    fn default() -> Self {
        CognitiveComplexity { report_level: 15 }
    }
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| v.to_string()).collect()
}
