//! The linter: reports rule violations found in the tree-sitter syntax tree.
//!
//! Rules cover what the formatter does not: layout is left to `jtk` formatting (see `docs/regel-analyse.md`).

mod bugs;
mod complexity;
mod design;
mod fix;
mod imports;
mod modernization;
mod naming;
mod options;
mod simplifications;
mod suppression;
mod syntax;
mod tree;

use std::path::Path;

use tree_sitter::Node;

use crate::formatter::FormatError;
use syntax::source;

/// Defines the rules with their code and the equivalent Checkstyle checks and PMD rules, whose suppressions also
/// apply to them. The PMD names are those of PMD 7.29, including the deprecated names PMD still accepts.
macro_rules! rules {
    ($($rule:ident $code:literal [$($checkstyle:literal),*] [$($pmd:literal),*],)*) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Rule {
            $($rule,)*
        }

        impl Rule {
            pub const ALL: &[Rule] = &[$(Rule::$rule,)*];

            pub fn code(self) -> &'static str {
                match self {
                    $(Rule::$rule => $code,)*
                }
            }

            fn checkstyle_names(self) -> &'static [&'static str] {
                match self {
                    $(Rule::$rule => &[$($checkstyle),*],)*
                }
            }

            fn pmd_names(self) -> &'static [&'static str] {
                match self {
                    $(Rule::$rule => &[$($pmd),*],)*
                }
            }
        }
    };
}

rules! {
    StarImport "I001" ["AvoidStarImport"] [],
    IllegalImport "I002" ["IllegalImport"] ["DontImportSun"],
    StaticImport "I003" ["AvoidStaticImport"] [],
    FullyQualifiedType "I005" ["UnnecessaryFullyQualifiedType"] ["UnnecessaryFullyQualifiedName"],
    EqualsHashCode "B001" ["EqualsHashCode"] ["OverrideBothEqualsAndHashcode"],
    CovariantEquals "B002" ["CovariantEquals"] [],
    EmptyCatch "B003" ["EmptyCatchBlock"] ["EmptyCatchBlock"],
    FallThrough "B004" ["FallThrough"] ["ImplicitSwitchFallThrough"],
    MissingSwitchDefault "B005" ["MissingSwitchDefault"] ["SwitchStmtsShouldHaveDefault", "NonExhaustiveSwitch"],
    DefaultComesLast "B006" ["DefaultComesLast"] ["DefaultLabelNotLastInSwitch"],
    StringLiteralEquality "B007" ["StringLiteralEquality"] ["UseEqualsToCompareStrings"],
    EqualsAvoidNull "B008" ["EqualsAvoidNull"] ["LiteralsFirstInComparisons"],
    EqualsNull "B009" [] ["EqualsNull"],
    FinallyExit "B010" [] ["ReturnFromFinallyBlock", "DoNotThrowExceptionInFinally"],
    IdenticalOperands "B011" [] [],
    InnerAssignment "B012" ["InnerAssignment"] ["AssignmentInOperand"],
    ParameterAssignment "B013" ["ParameterAssignment"] ["AvoidReassigningParameters"],
    ModifiedControlVariable "B014" ["ModifiedControlVariable"] ["AvoidReassigningLoopVariables"],
    JumbledIncrementer "B015" [] ["JumbledIncrementer"],
    NullCheck "B016" [] ["MisplacedNullCheck", "BrokenNullCheck"],
    BigDecimalDouble "B017" [] ["AvoidDecimalLiteralsInBigDecimalConstructor"],
    SuspiciousOctalEscape "B018" [] ["SuspiciousOctalEscape"],
    DoubleCheckedLocking "B019" [] ["DoubleCheckedLocking"],
    IllegalCatch "B020" ["IllegalCatch"] ["AvoidCatchingThrowable", "AvoidCatchingGenericException", "AvoidCatchingNPE"],
    IllegalThrows "B021" ["IllegalThrows"] ["AvoidThrowingRawExceptionTypes"],
    PrintStackTrace "B022" [] ["AvoidPrintStackTrace", "SystemPrintln"],
    LostCause "B023" ["NoCauseGetMessageInExceptionConstructor"] ["PreserveStackTrace"],
    MissingSuperCall "B024" ["SuperClone", "SuperFinalize"] ["ProperCloneImplementation", "FinalizeDoesNotCallSuperFinalize"],
    DoubleBraceInitialization "B025" ["AvoidDoubleBraceInitialization"] ["DoubleBraceInitialization"],
    UnusedLocalVariable "B026" ["UnusedLocalVariable"] ["UnusedLocalVariable", "UnusedAssignment"],
    UnusedPrivateMember "B027" ["UnusedPrivateField"] ["UnusedPrivateField", "UnusedPrivateMethod"],
    UnusedParameter "B028" [] ["UnusedFormalParameter"],
    PatternVariableAssignment "B031" ["PatternVariableAssignment"] [],
    MissingNullCase "B032" ["MissingNullCaseInSwitch"] [],
    Finalizer "B033" ["NoFinalizer"] [],
    Clone "B034" ["NoClone"] [],
    MissingBraces "S001" ["NeedBraces"] ["ControlStatementBraces"],
    SimplifyBooleanReturn "S002" ["SimplifyBooleanReturn"] ["SimplifyBooleanReturns"],
    SimplifyBooleanExpression "S003" ["SimplifyBooleanExpression"] ["SimplifyBooleanExpressions", "SimplifiedTernary"],
    CollapsibleIf "S004" [] ["CollapsibleIfStatements"],
    LocalBeforeReturn "S005" [] ["UnnecessaryLocalBeforeReturn"],
    UnnecessaryParentheses "S006" ["UnnecessaryParentheses"] ["UselessParentheses"],
    UnnecessarySemicolon "S007" [
        "EmptyStatement",
        "UnnecessarySemicolonAfterOuterTypeDeclaration",
        "UnnecessarySemicolonAfterTypeMemberDeclaration",
        "UnnecessarySemicolonInEnumeration",
        "UnnecessarySemicolonInTryWithResources"
    ] ["UnnecessarySemicolon", "EmptyControlStatement"],
    RedundantModifier "S008" ["RedundantModifier"] ["UnnecessaryModifier"],
    AnnotationAfterModifier "S009" ["ModifierOrder"] [],
    UnnecessaryReturn "S010" [] ["UnnecessaryReturn"],
    SuperConstructorCall "S011" ["AvoidNoArgumentSuperConstructorCall"] [],
    ExplicitInitialization "S012" ["ExplicitInitialization"] ["RedundantFieldInitializer"],
    MultipleDeclarations "S013" ["MultipleVariableDeclarations"] ["OneDeclarationPerLine"],
    ArrayTypeStyle "S014" ["ArrayTypeStyle"] [],
    LiteralCase "S015" ["UpperEll", "HexLiteralCase", "NumericalPrefixesInfixesSuffixesCharacterCase"] [],
    EmptyBlock "S017" ["EmptyBlock"] ["EmptyControlStatement", "UnconditionalIfStatement"],
    NestedBlock "S018" ["AvoidNestedBlocks"] [],
    UnnecessaryInstantiation "S019" [] ["StringInstantiation", "PrimitiveWrapperInstantiation"],
    NullCheckBeforeInstanceof "S020" ["UnnecessaryNullCheckWithInstanceOf"] ["SimplifyConditional"],
    ExpressionLambda "S021" ["ExpressionOverBlockLambda"] [],
    InstanceofPattern "M001" [] [],
    EnhancedSwitch "M002" ["UseEnhancedSwitch"] [],
    WhenGuard "M003" ["WhenShouldBeUsed"] [],
    DiamondOperator "M004" [] ["UseDiamondOperator"],
    RecordPatternTypeArguments "M005" ["UnnecessaryTypeArgumentsWithRecordPattern"] [],
    UseVar "M006" [] [],
    EnhancedFor "M007" [] ["ForLoopCanBeForeach"],
    UnnamedVariable "M008" [
        "UnusedCatchParameterShouldBeUnnamed",
        "UnusedLambdaParameterShouldBeUnnamed",
        "UnusedTryResourceShouldBeUnnamed"
    ] [],
    TextBlock "M009" [] [],
    IdenticalCatch "M010" [] ["IdenticalCatchBranches"],
    FinalParameters "M011" ["FinalParameters"] ["MethodArgumentCouldBeFinal"],
    FinalField "M012" [] ["ImmutableField"],
    MissingDeprecated "M013" ["MissingDeprecated"] [],
    SealedPermits "M014" ["SealedShouldHavePermitsList"] [],
    FinalLocalVariable "M015" ["FinalLocalVariable"] ["LocalVariableCouldBeFinal"],
    UnnecessaryPermits "M016" ["UnnecessaryPermitsClause"] [],
    TypeName "N001" ["TypeName"] ["ClassNamingConventions"],
    MethodName "N002" ["MethodName"] ["MethodNamingConventions"],
    MemberName "N003" ["MemberName"] ["FieldNamingConventions"],
    ConstantName "N004" ["ConstantName"] ["FieldNamingConventions"],
    StaticVariableName "N005" ["StaticVariableName"] ["FieldNamingConventions"],
    LocalVariableName "N006" ["LocalVariableName", "LocalFinalVariableName"] ["LocalVariableNamingConventions"],
    ParameterName "N007" ["ParameterName", "LambdaParameterName"] ["FormalParameterNamingConventions"],
    CatchParameterName "N008" ["CatchParameterName"] ["LocalVariableNamingConventions"],
    PatternVariableName "N009" ["PatternVariableName"] ["LocalVariableNamingConventions"],
    RecordComponentName "N010" ["RecordComponentName"] [],
    TypeParameterName "N011" [
        "ClassTypeParameterName",
        "MethodTypeParameterName",
        "InterfaceTypeParameterName",
        "RecordTypeParameterName"
    ] ["GenericsNaming"],
    PackageName "N012" ["PackageName"] ["PackageCase"],
    AbbreviationInName "N013" ["AbbreviationAsWordInName"] [],
    IllegalIdentifier "N014" ["IllegalIdentifierName"] [],
    UtilityClassConstructor "A001" ["HideUtilityClassConstructor"] ["InstantiableUtilityClass", "UseUtilityClass"],
    FinalClass "A002" ["FinalClass"] ["ClassWithOnlyPrivateConstructorsShouldBeFinal"],
    VisibilityModifier "A003" ["VisibilityModifier"] [],
    MutableException "A004" ["MutableException"] [],
    ConstantInterface "A005" ["InterfaceIsType"] ["ConstantsInInterface"],
    OneTopLevelClass "A006" ["OneTopLevelClass"] [],
    PackageDirectory "A007" ["PackageDeclaration"] [],
    OverloadOrder "A008" ["OverloadMethodsDeclarationOrder", "ConstructorsDeclarationGrouping"] [],
    ThrowsException "A010" [] ["SignatureDeclareThrowsException"],
    AbstractWithoutAbstract "A011" [] ["AbstractClassWithoutAbstractMethod"],
    DeclarationUsageDistance "A014" ["VariableDeclarationUsageDistance"] [],
    MagicNumber "A015" ["MagicNumber"] [],
    OuterTypeFilename "A017" ["OuterTypeFilename"] [],
    CyclomaticComplexity "C001" ["CyclomaticComplexity"] ["CyclomaticComplexity"],
    BooleanExpressionComplexity "C003" ["BooleanExpressionComplexity"] [],
    JavaNcss "C004" ["JavaNCSS"] ["NcssCount"],
    ExecutableStatementCount "C005" ["ExecutableStatementCount"] [],
    MethodLength "C006" ["MethodLength"] [],
    FileLength "C007" ["FileLength"] [],
    LambdaBodyLength "C008" ["LambdaBodyLength"] [],
    AnonInnerLength "C009" ["AnonInnerLength"] [],
    ParameterNumber "C010" ["ParameterNumber"] ["ExcessiveParameterList"],
    RecordComponentNumber "C011" ["RecordComponentNumber"] [],
    OuterTypeNumber "C013" ["OuterTypeNumber"] [],
    NestedIfDepth "C014" ["NestedIfDepth"] ["AvoidDeeplyNestedIfStmts"],
    NestedForDepth "C015" ["NestedForDepth"] [],
    NestedTryDepth "C016" ["NestedTryDepth"] [],
    LineLength "C020" ["LineLength"] [],
    ThrowsCount "C021" ["ThrowsCount"] [],
    CognitiveComplexity "C022" [] ["CognitiveComplexity"],
}

impl Rule {
    /// The Java version the code a rule suggests needs.
    fn java_version(self) -> u32 {
        match self {
            Rule::InstanceofPattern => 16,
            Rule::DiamondOperator => 7,
            Rule::EnhancedSwitch => 14,
            Rule::UseVar => 10,
            Rule::TextBlock => 15,
            Rule::IdenticalCatch => 7,
            Rule::EnhancedFor => 5,
            Rule::SealedPermits | Rule::UnnecessaryPermits => 17,
            Rule::WhenGuard | Rule::RecordPatternTypeArguments => 21,
            Rule::UnnamedVariable => 22,
            _ => 1,
        }
    }

    /// Whether the rule is checked when no rules are selected: rules with few false alarms that every project can
    /// follow (see `docs/regel-analyse.md`).
    pub fn is_default(self) -> bool {
        const DEFAULT: &[&str] = &[
            "B001", "B002", "B003", "B004", "B007", "B009", "B010", "B011", "B014", "B015", "B016", "B017", "B019",
            "B026", "B027", "S001", "S002", "S003", "S006", "S007", "S008", "M001", "M004",
        ];
        DEFAULT.contains(&self.code()) || self.code().starts_with('N') && self.code() <= "N012"
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub rule: Rule,
    /// Byte offset in the source.
    pub offset: usize,
    pub message: String,
    /// The edits that fix the violation; empty if there is no automatic fix.
    pub fix: Vec<Edit>,
    /// Whether the fix may change the code's behaviour in edge cases: it is only applied when asked for.
    pub unsafe_fix: bool,
}

impl Diagnostic {
    fn new(rule: Rule, offset: usize, message: impl Into<String>) -> Diagnostic {
        Diagnostic { rule, offset, message: message.into(), fix: Vec::new(), unsafe_fix: false }
    }

    fn with_fix(mut self, edits: Vec<Edit>) -> Diagnostic {
        self.fix = edits;
        self
    }

    /// A fix that may change the behaviour, for example where types would tell whether it is correct.
    fn with_unsafe_fix(mut self, edits: Vec<Edit>) -> Diagnostic {
        self.fix = edits;
        self.unsafe_fix = true;
        self
    }
}

impl Edit {
    fn new(range: std::ops::Range<usize>, replacement: impl Into<String>) -> Edit {
        Edit { range, replacement: replacement.into() }
    }
}

/// Replaces a byte range of the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub range: std::ops::Range<usize>,
    pub replacement: String,
}

pub use fix::{apply_fix, fix, fix_with};
pub use options::RuleOptions;

/// The enabled rules: those matching a `select` prefix (the default rules if there are none, all rules for `ALL`) or
/// an `extend` prefix and no `ignore` prefix, and whose suggestions the project's Java version supports (all if it is
/// not given).
#[derive(Debug, Clone, Default)]
pub struct Selection {
    select: Vec<String>,
    extend: Vec<String>,
    ignore: Vec<String>,
    java_version: Option<u32>,
}

impl Selection {
    /// Fails on a code prefix that matches no rule, which is most likely a typo.
    pub fn new(select: Vec<String>, ignore: Vec<String>) -> Result<Selection, String> {
        let unknown = select
            .iter()
            .chain(&ignore)
            .find(|prefix| *prefix != "ALL" && !Rule::ALL.iter().any(|r| r.code().starts_with(*prefix)));
        match unknown {
            Some(prefix) => Err(format!("no rule matches '{prefix}'")),
            None => Ok(Selection { select, extend: Vec::new(), ignore, java_version: None }),
        }
    }

    /// Also enables the rules matching the prefixes, in addition to the selected or default ones.
    pub fn extended(self, extend: Vec<String>) -> Result<Selection, String> {
        let checked = Selection::new(extend, Vec::new())?;
        Ok(Selection { extend: checked.select, ..self })
    }

    pub fn with_java_version(self, java_version: u32) -> Selection {
        Selection { java_version: Some(java_version), ..self }
    }

    fn includes(&self, rule: Rule) -> bool {
        let matches = |prefix: &String| prefix == "ALL" || rule.code().starts_with(prefix.as_str());
        let selected = if self.select.is_empty() { rule.is_default() } else { self.select.iter().any(matches) };
        let selected = selected || self.extend.iter().any(matches);
        let supported = self.java_version.is_none_or(|v| rule.java_version() <= v);
        selected && supported && !self.ignore.iter().any(matches)
    }
}

/// The selected rules and their parameters.
#[derive(Debug, Clone, Default)]
pub struct Settings {
    pub selection: Selection,
    pub options: RuleOptions,
}

impl From<Selection> for Settings {
    fn from(selection: Selection) -> Settings {
        Settings { selection, options: RuleOptions::default() }
    }
}

/// Returns the violations of the selected rules (with their default parameters) in a compilation unit that are not
/// suppressed, ordered by position.
pub fn lint(text: &str, selection: &Selection) -> Result<Vec<Diagnostic>, FormatError> {
    lint_file(text, None, &Settings::from(selection.clone()))
}

/// Like [`lint`] with the given rule parameters, and also checks the rules about the file's path (if known), such
/// as whether its name matches the type it declares.
pub fn lint_file(text: &str, path: Option<&Path>, settings: &Settings) -> Result<Vec<Diagnostic>, FormatError> {
    let tree = crate::parse::parse(text).ok_or_else(|| FormatError::new("parse failed"))?;
    let root = tree.root_node();
    if root.has_error() {
        let error = crate::ast::first_error(&root);
        return Err(FormatError::at_offset(text, error.start_byte(), "syntax error"));
    }
    let mut diagnostics = Vec::new();
    let checks = checks_by_kind(&root.language(), &settings.selection);
    for_each_node(&root, |node| {
        if let Some(checks) = checks.get(usize::from(node.kind_id())) {
            for check in checks {
                match check {
                    AnyCheck::Plain(check) => check(node, text, &mut diagnostics),
                    AnyCheck::Configured(check) => check(node, text, &settings.options, &mut diagnostics),
                }
            }
        }
    });
    let unit = crate::parser::parse_cached(text)
        .map_or_else(|| crate::ast::Converter { src: text }.compilation_unit(&root).map(std::rc::Rc::new), Ok)?;
    let mut context = tree::Context::new(text, &settings.options, path);
    tree::check(&unit, &mut context, &settings.selection, &mut diagnostics);
    if let Some(path) = path {
        for (rule, check) in design::FILE_CHECKS {
            if settings.selection.includes(*rule) {
                check(&root, text, path, &mut diagnostics);
            }
        }
    }
    let suppressions = suppression::Suppressions::find(&root, text);
    diagnostics.retain(|d| !suppressions.suppresses(d));
    diagnostics.sort_by_key(|d| d.offset);
    Ok(diagnostics)
}

/// Calls `f` for every node in document order. Iterative, so that deeply nested code cannot overflow the stack.
fn for_each_node<'t>(root: &Node<'t>, mut f: impl FnMut(&Node<'t>)) {
    let mut cursor = root.walk();
    loop {
        f(&cursor.node());
        if cursor.goto_first_child() || cursor.goto_next_sibling() {
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return;
            }
        }
    }
}

type Check = fn(&Node, &str, &mut Vec<Diagnostic>);

/// The check of a rule with parameters.
type ConfiguredCheck = fn(&Node, &str, &RuleOptions, &mut Vec<Diagnostic>);

#[derive(Clone, Copy)]
enum AnyCheck {
    Plain(Check),
    Configured(ConfiguredCheck),
}

/// A check of the whole compilation unit (the `program` node) against the file's path.
type FileCheck = fn(&Node, &str, &Path, &mut Vec<Diagnostic>);

/// The groups' checks: each rule with the kinds of nodes it checks.
const CHECKS: &[&[(Rule, &[&str], Check)]] =
    &[imports::CHECKS, bugs::CHECKS, simplifications::CHECKS, modernization::CHECKS, naming::CHECKS, design::CHECKS];

/// The groups' checks of rules with parameters.
const CONFIGURED_CHECKS: &[&[(Rule, &[&str], ConfiguredCheck)]] = &[
    imports::CONFIGURED_CHECKS,
    bugs::CONFIGURED_CHECKS,
    naming::CONFIGURED_CHECKS,
    design::CONFIGURED_CHECKS,
    complexity::CONFIGURED_CHECKS,
];

/// The checks of the selected rules, indexed by the id of the node kind they check.
fn checks_by_kind(language: &tree_sitter::Language, selection: &Selection) -> Vec<Vec<AnyCheck>> {
    let plain = CHECKS.iter().copied().flatten().map(|(rule, kinds, check)| (*rule, *kinds, AnyCheck::Plain(*check)));
    let configured = CONFIGURED_CHECKS
        .iter()
        .copied()
        .flatten()
        .map(|(rule, kinds, check)| (*rule, *kinds, AnyCheck::Configured(*check)));
    let mut checks: Vec<Vec<AnyCheck>> = vec![Vec::new(); language.node_kind_count()];
    for (rule, kinds, check) in plain.chain(configured) {
        if !selection.includes(rule) {
            continue;
        }
        for kind in kinds {
            let id = language.id_for_node_kind(kind, true);
            assert!(id != 0, "unknown node kind {kind}");
            checks[usize::from(id)].push(check);
        }
    }
    checks
}
