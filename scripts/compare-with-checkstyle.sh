#!/usr/bin/env bash
# Runs Checkstyle checks and the equivalent jtk rules on a source tree, and compares the reported lines per rule.
#
# Usage: scripts/compare-with-checkstyle.sh <checkstyle-all.jar> <source-dir> [work-dir] [code-prefixes]
#   code-prefixes: compare only these rules, e.g. B005,S01 (default: all rules in the mapping below).
#   The jar is published with each Checkstyle release on GitHub (checkstyle-<version>-all.jar).
#   The Checkstyle test inputs (src/test/resources/com/puppycrawl/tools/checkstyle/checks) make a good corpus.
#
# Prints, per rule: violations found by Checkstyle, by jtk, and on the same line. Lists of the differences are left
# in the work directory. Checkstyle stops at the first file it fails on, so the corpus should only hold files both
# tools can parse.
set -euo pipefail

jar=$(realpath "$1")
src=$(cd "$2" && pwd)
work=${3:-$(mktemp -d)}
prefixes=${4:-}
port=$(cd "$(dirname "$0")/.." && pwd)/target/release/jtk

# jtk rule code, Checkstyle module and the properties that give it the same scope as the jtk rule.
mapping='
B001 EqualsHashCode
B002 CovariantEquals
B003 EmptyCatchBlock
B004 FallThrough
B007 StringLiteralEquality
B014 ModifiedControlVariable
B026 UnusedLocalVariable
B027 UnusedPrivateField
S001 NeedBraces tokens=LITERAL_DO,LITERAL_ELSE,LITERAL_FOR,LITERAL_IF,LITERAL_WHILE
S002 SimplifyBooleanReturn
S003 SimplifyBooleanExpression
S006 UnnecessaryParentheses
S007 EmptyStatement
S007 UnnecessarySemicolonAfterOuterTypeDeclaration
S007 UnnecessarySemicolonAfterTypeMemberDeclaration
S007 UnnecessarySemicolonInEnumeration
S007 UnnecessarySemicolonInTryWithResources
S008 RedundantModifier
N001 TypeName
N002 MethodName
N003 MemberName
N004 ConstantName
N005 StaticVariableName
N006 LocalVariableName
N006 LocalFinalVariableName
N007 ParameterName
N007 LambdaParameterName
N008 CatchParameterName
N009 PatternVariableName
N010 RecordComponentName
N011 ClassTypeParameterName
N011 MethodTypeParameterName
N011 InterfaceTypeParameterName
N011 RecordTypeParameterName
N012 PackageName
I001 AvoidStarImport
I002 IllegalImport
I003 AvoidStaticImport
I005 UnnecessaryFullyQualifiedType
B005 MissingSwitchDefault
B006 DefaultComesLast
B008 EqualsAvoidNull
B012 InnerAssignment
B013 ParameterAssignment
B020 IllegalCatch
B021 IllegalThrows
B023 NoCauseGetMessageInExceptionConstructor
B024 SuperClone
B024 SuperFinalize
B025 AvoidDoubleBraceInitialization
B031 PatternVariableAssignment
B032 MissingNullCaseInSwitch
B033 NoFinalizer
B034 NoClone
S009 ModifierOrder
S011 AvoidNoArgumentSuperConstructorCall
S012 ExplicitInitialization
S013 MultipleVariableDeclarations
S014 ArrayTypeStyle
S015 UpperEll
S015 HexLiteralCase
S015 NumericalPrefixesInfixesSuffixesCharacterCase
S017 EmptyBlock
S018 AvoidNestedBlocks
S020 UnnecessaryNullCheckWithInstanceOf
S021 ExpressionOverBlockLambda
M002 UseEnhancedSwitch
M003 WhenShouldBeUsed
M005 UnnecessaryTypeArgumentsWithRecordPattern
M008 UnusedCatchParameterShouldBeUnnamed
M008 UnusedLambdaParameterShouldBeUnnamed
M008 UnusedTryResourceShouldBeUnnamed
M011 FinalParameters
M013 MissingDeprecated
M014 SealedShouldHavePermitsList
M015 FinalLocalVariable
M016 UnnecessaryPermitsClause
N013 AbbreviationAsWordInName
N014 IllegalIdentifierName
A001 HideUtilityClassConstructor
A002 FinalClass
A003 VisibilityModifier
A004 MutableException
A005 InterfaceIsType
A006 OneTopLevelClass
A007 PackageDeclaration
A008 OverloadMethodsDeclarationOrder
A008 ConstructorsDeclarationGrouping
A014 VariableDeclarationUsageDistance
A015 MagicNumber
A017 OuterTypeFilename
C001 CyclomaticComplexity
C003 BooleanExpressionComplexity
C004 JavaNCSS
C005 ExecutableStatementCount
C006 MethodLength
C007 FileLength
C008 LambdaBodyLength
C009 AnonInnerLength
C010 ParameterNumber
C011 RecordComponentNumber
C013 OuterTypeNumber
C014 NestedIfDepth
C015 NestedForDepth
C016 NestedTryDepth
C021 ThrowsCount
'

if [ -n "$prefixes" ]; then
  mapping=$(echo "$mapping" | awk -v p="$prefixes" 'NF { n = split(p, ps, ","); for (i = 1; i <= n; i++) if (index($1, ps[i]) == 1) { print; next } }')
fi

# Checks that Checkstyle runs on the file text rather than in the TreeWalker.
checker_modules=' FileLength '

module_config() {
  echo "$mapping" | while read -r code module properties; do
    [ -n "$code" ] || continue
    case "$checker_modules" in *" $module "*) [ "$1" = checker ] || continue ;; *) [ "$1" = tree ] || continue ;; esac
    printf '<module name="%s">' "$module"
    [ "$1" = checker ] && printf '<property name="fileExtensions" value="java"/>'
    for property in $properties; do printf '<property name="%s" value="%s"/>' "${property%%=*}" "${property#*=}"; done
    echo '</module>'
  done
}

cargo build --release --quiet --manifest-path "$(dirname "$0")/../Cargo.toml"
mkdir -p "$work"

{
  echo '<?xml version="1.0"?>'
  echo '<!DOCTYPE module PUBLIC "-//Checkstyle//DTD Checkstyle Configuration 1.3//EN" "https://checkstyle.org/dtds/configuration_1_3.dtd">'
  echo '<module name="Checker"><property name="severity" value="warning"/>'
  module_config checker
  echo '<module name="TreeWalker">'
  module_config tree
  echo '</module></module>'
} > "$work/checkstyle.xml"

codes_of_module=$(echo "$mapping" | awk 'NF { print "s/\\[" $2 "\\]$/" $1 "/" }')
(cd "$src" && java -jar "$jar" -c "$work/checkstyle.xml" . 2>"$work/checkstyle-errors.txt" || true) \
  | grep '^\[WARN\]' | sed -E "s#^\[WARN\] ##; s#^$src/##; s#^\./##" | sed "$codes_of_module" \
  | sed -E 's/^([^:]+):([0-9]+):.* ([A-Z][0-9]+)$/\3 \1:\2/' | sort -u > "$work/checkstyle.txt"

codes=$(echo "$mapping" | awk 'NF { print $1 }' | sort -u | paste -sd,)
(cd "$src" && find . -name '*.java' -print0 | xargs -0 "$port" lint --select "$codes" 2>"$work/jtk-errors.txt" || true) \
  | sed -E 's#^\./##; s/^([^:]+):([0-9]+):[0-9]+: ([A-Z][0-9]+) .*/\3 \1:\2/' | sort -u > "$work/jtk.txt"

printf '%-6s %10s %6s %10s\n' rule checkstyle jtk same-line
for code in ${codes//,/ }; do
  printf '%-6s %10s %6s %10s\n' "$code" "$(grep -c "^$code " "$work/checkstyle.txt" || true)" \
    "$(grep -c "^$code " "$work/jtk.txt" || true)" \
    "$(comm -12 <(grep "^$code " "$work/checkstyle.txt" || true) <(grep "^$code " "$work/jtk.txt" || true) | wc -l)"
done
comm -23 "$work/checkstyle.txt" "$work/jtk.txt" > "$work/only-checkstyle.txt"
comm -13 "$work/checkstyle.txt" "$work/jtk.txt" > "$work/only-jtk.txt"
echo "errors (checkstyle): $(grep -c Exception "$work/checkstyle-errors.txt" || true), errors (jtk): $(grep -c . "$work/jtk-errors.txt" || true)"
echo "differences in $work/only-checkstyle.txt and $work/only-jtk.txt"
