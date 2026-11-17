#!/usr/bin/env bash
# Runs PMD rules and the equivalent jtk rules on a source tree, and compares the reported lines per rule.
#
# Usage: scripts/compare-with-pmd.sh <pmd-bin-dir> <source-dir> [work-dir] [code-prefixes]
#   pmd-bin-dir: an unpacked PMD 7 binary distribution (pmd-dist-<version>-bin.zip from PMD's GitHub releases).
#   code-prefixes: compare only these rules, e.g. B009,S0 (default: all rules in the mapping below).
#   scripts/extract-pmd-test-code.py turns PMD's rule tests into a good corpus.
#
# Prints, per rule: violations found by PMD, by jtk, and on the same line. Lists of the differences are left in the
# work directory. PMD runs without the project's classpath, so rules that need type information may find less.
set -euo pipefail

pmd=$(cd "$1" && pwd)/bin/pmd
src=$(cd "$2" && pwd)
work=${3:-$(mktemp -d)}
prefixes=${4:-}
port=$(cd "$(dirname "$0")/.." && pwd)/target/release/jtk

# jtk rule code, PMD rule (category/name) and the properties that give it the same scope as the jtk rule. Only rules
# with comparable semantics are listed: PMD's naming conventions, metrics and AvoidDuplicateLiterals use other
# defaults than the Checkstyle checks jtk follows.
mapping='
B001 errorprone/OverrideBothEqualsAndHashcode
B003 errorprone/EmptyCatchBlock allowCommentedBlocks=true
B004 errorprone/ImplicitSwitchFallThrough
B005 bestpractices/NonExhaustiveSwitch
B006 bestpractices/DefaultLabelNotLastInSwitch
B007 errorprone/UseEqualsToCompareStrings
B008 bestpractices/LiteralsFirstInComparisons
B009 errorprone/EqualsNull
B010 errorprone/ReturnFromFinallyBlock
B010 errorprone/DoNotThrowExceptionInFinally
B012 errorprone/AssignmentInOperand
B013 bestpractices/AvoidReassigningParameters
B014 bestpractices/AvoidReassigningLoopVariables
B015 errorprone/JumbledIncrementer
B016 errorprone/MisplacedNullCheck
B016 errorprone/BrokenNullCheck
B017 errorprone/AvoidDecimalLiteralsInBigDecimalConstructor
B019 multithreading/DoubleCheckedLocking
B020 errorprone/AvoidCatchingGenericException
B020 errorprone/AvoidCatchingThrowable
B020 errorprone/AvoidCatchingNPE
B021 design/AvoidThrowingRawExceptionTypes
B023 bestpractices/PreserveStackTrace
B024 errorprone/ProperCloneImplementation
B024 errorprone/FinalizeDoesNotCallSuperFinalize
B025 bestpractices/DoubleBraceInitialization
B026 bestpractices/UnusedLocalVariable
B027 bestpractices/UnusedPrivateField
B027 bestpractices/UnusedPrivateMethod
S001 codestyle/ControlStatementBraces
S002 design/SimplifyBooleanReturns
S003 design/SimplifyBooleanExpressions
S003 design/SimplifiedTernary
S006 codestyle/UselessParentheses
S007 codestyle/UnnecessarySemicolon
S008 codestyle/UnnecessaryModifier
S012 performance/RedundantFieldInitializer
S013 bestpractices/OneDeclarationPerLine
S017 codestyle/EmptyControlStatement
S017 errorprone/UnconditionalIfStatement
S020 design/SimplifyConditional
M004 codestyle/UseDiamondOperator
M011 codestyle/MethodArgumentCouldBeFinal
M015 codestyle/LocalVariableCouldBeFinal
A001 design/InstantiableUtilityClass
A002 design/ClassWithOnlyPrivateConstructorsShouldBeFinal
A005 bestpractices/ConstantsInInterface
B018 errorprone/SuspiciousOctalEscape
B022 bestpractices/AvoidPrintStackTrace
B022 bestpractices/SystemPrintln
B028 bestpractices/UnusedFormalParameter
S004 design/CollapsibleIfStatements
S005 codestyle/UnnecessaryLocalBeforeReturn
S010 codestyle/UnnecessaryReturn
S019 performance/StringInstantiation
S019 bestpractices/PrimitiveWrapperInstantiation
M007 bestpractices/ForLoopCanBeForeach
M010 codestyle/IdenticalCatchBranches
M012 design/ImmutableField
A010 design/SignatureDeclareThrowsException
A011 bestpractices/AbstractClassWithoutAbstractMethod
C022 design/CognitiveComplexity
'

if [ -n "$prefixes" ]; then
  mapping=$(echo "$mapping" | awk -v p="$prefixes" 'NF { n = split(p, ps, ","); for (i = 1; i <= n; i++) if (index($1, ps[i]) == 1) { print; next } }')
fi

cargo build --release --quiet --manifest-path "$(dirname "$0")/../Cargo.toml"
mkdir -p "$work"

{
  echo '<?xml version="1.0"?>'
  echo '<ruleset name="jtk" xmlns="http://pmd.sourceforge.net/ruleset/2.0.0"'
  echo '    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"'
  echo '    xsi:schemaLocation="http://pmd.sourceforge.net/ruleset/2.0.0 https://pmd.sourceforge.io/ruleset_2_0_0.xsd">'
  echo '  <description>Rules compared with jtk</description>'
  echo "$mapping" | while read -r code rule properties; do
    [ -n "$code" ] || continue
    printf '  <rule ref="category/java/%s.xml/%s">' "${rule%%/*}" "${rule#*/}"
    if [ -n "$properties" ]; then
      printf '<properties>'
      for property in $properties; do printf '<property name="%s" value="%s"/>' "${property%%=*}" "${property#*=}"; done
      printf '</properties>'
    fi
    echo '</rule>'
  done
  echo '</ruleset>'
} > "$work/ruleset.xml"

# PMD's text format: <file>:<line>:<tab><rule>:<tab><message>
codes_of_rule=$(echo "$mapping" | awk 'NF { split($2, r, "/"); print "s/^" r[2] " /" $1 " /" }')
(cd "$src" && "$pmd" check --no-cache --no-progress -R "$work/ruleset.xml" -d . -f text 2>"$work/pmd-errors.txt" || true) \
  | sed -E "s#^$src/##; s#^\./##" | sed -nE 's/^([^:]+):([0-9]+):\t([A-Za-z]+):\t.*/\3 \1:\2/p' \
  | sed "$codes_of_rule" | sort -u > "$work/pmd.txt"

codes=$(echo "$mapping" | awk 'NF { print $1 }' | sort -u | paste -sd,)
(cd "$src" && find . -name '*.java' -print0 | xargs -0 "$port" lint --select "$codes" 2>"$work/jtk-errors.txt" || true) \
  | sed -E 's#^\./##; s/^([^:]+):([0-9]+):[0-9]+: ([A-Z][0-9]+) .*/\3 \1:\2/' | sort -u > "$work/jtk.txt"

printf '%-6s %6s %6s %10s\n' rule pmd jtk same-line
for code in ${codes//,/ }; do
  printf '%-6s %6s %6s %10s\n' "$code" "$(grep -c "^$code " "$work/pmd.txt" || true)" \
    "$(grep -c "^$code " "$work/jtk.txt" || true)" \
    "$(comm -12 <(grep "^$code " "$work/pmd.txt" || true) <(grep "^$code " "$work/jtk.txt" || true) | wc -l)"
done
comm -23 "$work/pmd.txt" "$work/jtk.txt" > "$work/only-pmd.txt"
comm -13 "$work/pmd.txt" "$work/jtk.txt" > "$work/only-jtk.txt"
echo "files PMD could not process: $(grep -c 'Error while' "$work/pmd-errors.txt" || true), jtk errors: $(grep -c . "$work/jtk-errors.txt" || true)"
echo "differences in $work/only-pmd.txt and $work/only-jtk.txt"
