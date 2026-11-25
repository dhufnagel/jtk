# Regelanalyse: `jtk fmt` und `jtk lint`

Stand: 16.11.2026 · jtk `c1c43ff` · Checkstyle 14.4.0 · Quellen: PMD 7, Checkstyle, Error Prone, Eclipse-Save-Actions

**Nachtrag 25.11.2026 (jtk 0.8.0):** M002, M006, M007, M009 und M010 sind ins Standard-Set gekommen (★), damit `jtk`
ohne Befehl die Eclipse-Save-Actions ausführt; M009 hat seither einen sicheren Fix. Die Regeln aus weiteren Tools,
darunter die nach einem Abgleich mit SonarQube ergänzten, stehen in [regel-erweiterung.md](regel-erweiterung.md).

## 1. Ausgangslage

`jtk fmt` ist ein byte-identischer Port von palantir-java-format 2.103.0. Das ist das stärkste Argument für das Tool und
setzt die Grenze: **fmt bekommt keine eigenen Regeln, die von Palantir abweichen.** Was fmt heute schon tut:

| Funktion | Modul | Ersetzt |
|---|---|---|
| Layout: Einrückung (4/8), Umbrüche (120 Spalten), Klammern, Whitespace, Leerzeilen (maximal eine) | `visitor.rs`, `doc.rs` | alle Whitespace-, Klammer- und Einrückungs-Checks |
| Imports sortieren (Palantir-Reihenfolge) | `imports.rs` | CS ImportOrder/CustomImportOrder, Eclipse „Organize imports“ |
| Unbenutzte Imports entfernen, inkl. `java.lang.*` und Imports aus dem eigenen Package | `imports.rs` | CS UnusedImports/RedundantImport, PMD UnnecessaryImport, EP RemoveUnusedImports |
| Modifier-Keywords in JLS-Reihenfolge | `modifiers.rs` | CS ModifierOrder (nur Keywords, nicht die Position von Annotationen) |
| Lange Strings umbrechen, Text Blocks einrücken | `string_wrapper.rs` | – |

Nicht enthalten: Javadoc-Formatierung (in Palantir standardmäßig aus), Star-Imports auflösen, Imports ergänzen.

## 2. Messung

Korpora: Checkstyle `src/main/java` (504 Dateien) und commons-lang `src/main/java` (264 Dateien), jeweils vor und
nach `jtk -i`. Darauf liefen alle 231 Checkstyle-Checks mit Standardeinstellungen, ohne die Checks, die Dateien
oder Pflicht-Properties brauchen.

**Ergebnis:** Nur Layout-Checks ändern ihre Trefferzahl. Bei allen inhaltlichen Checks bleibt die Zahl vor und nach
fmt identisch. fmt und lint überschneiden sich also nicht.

| Wirkung von fmt | Checks (echter Code, Treffer vorher → nachher) |
|---|---|
| **Vollständig behoben** | GoogleRightCurly 944→0, ImportOrder 658→2, CustomImportOrder 402→1, OpenjdkMethodParameterAlignment 150→0, NoWhitespaceAfter 60→0, OperatorWrap 42→0, SingleSpaceSeparator 30→0, JavadocLeadingAsteriskAlign 27→0, CommentsIndentation 14→0, JavadocMissingLeadingAsterisk 7→0 |
| **Weitgehend behoben** | RightCurly 969→26 (Kommentar zwischen `}` und `else`), Indentation 1001→167 (Rest: Konflikt bei der Fortsetzungseinrückung) |
| **Konflikt mit Palantir** (fmt erzeugt die Treffer) | EmptyLineWrappingInBlock 324→1012, TypeBodyPadding 156→711, WhitespaceAround 0→165 (`{}`), LineLength 20840→22335 (Standard 80 statt 120), OpenjdkMethodThrowsAlignment 7→32, EmptyForIteratorPad 0→21, SeparatorWrap 573→530 (Standard `.` am Zeilenende) |
| **Unverändert** | alle übrigen ~200 Checks: Naming, Coding, Design, Metrics, inhaltliches Javadoc, Imports (Star/Illegal) |

Folgerung für den Config-Import aus `checkstyle.xml`: Layout-Checks werden **nicht** als Lint-Regeln übernommen,
sondern als „abgedeckt durch `jtk fmt`“ bzw. „inkompatibel mit dem Palantir-Stil“ gemeldet.

## 3. Grundsatz

| | `jtk fmt` | `jtk lint` |
|---|---|---|
| Aufgabe | Layout, byte-identisch zu Palantir | Bedeutung, Struktur, Konventionen |
| Konfiguration | keine | Regelauswahl, Schwellwerte, Java-Zielversion |
| Änderungen | immer | nur mit `--fix` (safe) bzw. `--unsafe-fixes` |
| Token-Änderungen (Klammern, Kommas, Literale) | nur wo Palantir sie macht | als Fix |

Wie bei ruff (`ruff check --fix && ruff format`): Fixes ändern die Struktur, fmt danach das Layout. Ein Fix muss
also kein schönes Layout erzeugen, nur gültigen Code. Bequemer Einstieg: `jtk lint --fix` führt am Ende automatisch
fmt auf den geänderten Dateien aus.

## 4. `jtk fmt`

### Bereits abgedeckt (kein Handlungsbedarf)
Alle Whitespace-, Klammer-, Einrückungs- und Umbruch-Regeln, Import-Reihenfolge, unbenutzte und redundante Imports,
die Reihenfolge der Modifier-Keywords, eine Anweisung pro Zeile. Aus den Eclipse-Save-Actions: *Format source code*,
*Organize imports* (ohne Ergänzen), *Remove trailing whitespace*, *Correct indentation*.

### Sinnvolle Ergänzung: Javadoc-Formatierung (opt-in)
Die einzige Ergänzung, die zum Konzept passt. palantir/google-java-format haben sie als Option (`JavadocFormatter`),
fmt bliebe also kompatibel. Sie würde diese Checks erledigen, die heute unverändert bleiben:
JavadocParagraph (1704 Treffer im echten Code), JavadocTagContinuationIndentation (212),
JavadocMissingWhitespaceAfterAsterisk, SingleLineJavadoc, RequireEmptyLineBeforeBlockTagGroup, JavadocContentLocation.

### Bewusst nicht in fmt (stattdessen Lint-Fix)
Trailing Commas, Groß-/Kleinschreibung von Literalen (`1l`, `0XFF`), `int a[]`, fehlende Klammern bei `if`,
unnötige Klammern oder Semikolons, Annotationen zwischen Modifiern, Leerzeilen zwischen Membern erzwingen, Members
sortieren. All das würde die Byte-Identität mit Palantir brechen. Beobachtung dazu: fmt lässt ein überflüssiges `;`
stehen (`void run() {};` wird zu `{}` plus `;` in einer eigenen Zeile). Das zeigt, warum `UnnecessarySemicolon` als
Lint-Fix nützlich ist.

## 5. `jtk lint`

Legende: ★ im Standard-Regelset · Fix ✅ safe / ⚠️ unsafe / – keiner · Stufe 1 = rein syntaktisch, 2 = Symboltabelle
innerhalb der Datei, 3 = Metriken/Javadoc/Konfigurierbares. Herkunft: CS = Checkstyle, EP = Error Prone,
EC = Eclipse-Save-Action.

### I: Imports (der Rest, den fmt nicht abdeckt)
| Code | Regel | Herkunft | Fix | Stufe |
|---|---|---|---|---|
| I001 | Star-Import | CS AvoidStarImport | – | 1 |
| I002 | Verbotener Import (`sun.*`, konfigurierbar) | CS IllegalImport, PMD DontImportSun | – | 1 |
| I003 | Statische Imports einschränken | CS AvoidStaticImport | – | 1 |
| I004 | Architekturregeln (Import-Kontrolle) | CS ImportControl | – | 3 |
| I005 | Unnötig voll qualifizierter Typ | CS UnnecessaryFullyQualifiedType, PMD UnnecessaryFullyQualifiedName | ✅ | 2 |

### B: Fehlerquellen
| Code | Regel | Herkunft | Fix | Stufe |
|---|---|---|---|---|
| B001 ★ | `equals` ohne `hashCode` | CS EqualsHashCode, PMD OverrideBothEqualsAndHashcode | – | 1 |
| B002 ★ | `equals(Foo)` statt `equals(Object)` | CS CovariantEquals | – | 1 |
| B003 ★ | Leerer `catch`-Block | CS EmptyCatchBlock, PMD EmptyCatchBlock, EP EmptyCatch | – | 1 |
| B004 ★ | Fallthrough im `switch` | CS FallThrough, PMD ImplicitSwitchFallThrough, EP FallThrough | – | 1 |
| B005 | Fehlendes `default` | CS MissingSwitchDefault, PMD NonExhaustiveSwitch, EP MissingDefault | – | 1 |
| B006 | `default` nicht zuletzt | CS DefaultComesLast | – | 1 |
| B007 ★ | `==` mit String-Literal | CS StringLiteralEquality, PMD UseEqualsToCompareStrings | ⚠️ | 1 |
| B008 | Literal zuerst bei `equals` | CS EqualsAvoidNull, PMD LiteralsFirstInComparisons | ⚠️ | 1 |
| B009 ★ | `x.equals(null)` | PMD EqualsNull | ⚠️ | 1 |
| B010 ★ | `return`/`throw` in `finally` | PMD ReturnFromFinallyBlock, DoNotThrowExceptionInFinally | – | 1 |
| B011 ★ | Selbstzuweisung, `a == a` | EP SelfAssignment, IdentityBinaryExpression | – | 1 |
| B012 | Zuweisung in Ausdrücken | CS InnerAssignment, PMD AssignmentInOperand | – | 1 |
| B013 | Parameter neu zugewiesen | CS ParameterAssignment, PMD AvoidReassigningParameters | – | 1 |
| B014 ★ | Schleifenvariable verändert | CS ModifiedControlVariable, PMD AvoidReassigningLoopVariables | – | 1 |
| B015 ★ | Verwechselter Inkrement | PMD JumbledIncrementer | – | 1 |
| B016 ★ | Falsche/defekte Null-Prüfung | PMD MisplacedNullCheck, BrokenNullCheck | – | 1 |
| B017 ★ | `new BigDecimal(0.1)` | PMD AvoidDecimalLiteralsInBigDecimalConstructor | ⚠️ | 1 |
| B018 | Verdächtiges Oktal-Escape | PMD SuspiciousOctalEscape | – | 1 |
| B019 ★ | Double-Checked Locking | PMD DoubleCheckedLocking | – | 1 |
| B020 | `catch (Throwable/Exception/NPE)` | CS IllegalCatch, PMD AvoidCatchingThrowable/AvoidCatchingNPE | – | 1 |
| B021 | `throw new RuntimeException/Exception` | CS IllegalThrows, PMD AvoidThrowingRawExceptionTypes | – | 1 |
| B022 | `printStackTrace()`, `System.out/err` | PMD AvoidPrintStackTrace, SystemPrintln, EP CatchAndPrintStackTrace | – | 1 |
| B023 | `new X(e.getMessage())` statt `new X(…, e)` | CS NoCauseGetMessageInExceptionConstructor, PMD PreserveStackTrace | – | 2 |
| B024 | `clone()`/`finalize()` ohne `super`-Aufruf | CS SuperClone, SuperFinalize | – | 1 |
| B025 | Double-Brace-Initialisierung | CS AvoidDoubleBraceInitialization | – | 1 |
| B026 ★ | Unbenutzte lokale Variable / Zuweisung | CS UnusedLocalVariable, PMD UnusedLocalVariable/UnusedAssignment, EP UnusedVariable | ⚠️ | 2 |
| B027 ★ | Unbenutztes privates Feld / private Methode | CS UnusedPrivateField, PMD UnusedPrivateField/UnusedPrivateMethod, EP UnusedMethod | ⚠️ | 2 |
| B028 | Unbenutzter Parameter (private Methoden) | PMD UnusedFormalParameter | – | 2 |
| ~~B029~~ | nicht umgesetzt: Feld verdeckt durch Variable/Parameter | CS HiddenField | – | – |
| B030 | Operator-Präzedenz unklar (`a && b \|\| c`) | EP OperatorPrecedence | ✅ | 1 |
| B031 | Pattern-Variable neu zugewiesen | CS PatternVariableAssignment | – | 1 |
| B032 | Fehlender `case null` bei Pattern-Switch | CS MissingNullCaseInSwitch | – | 1 |
| B033 | `finalize()` überhaupt deklariert | CS NoFinalizer | – | 1 |
| B034 | `clone()` überhaupt deklariert | CS NoClone | – | 1 |

### S: Vereinfachungen (fast alle mit Fix, ergänzen fmt am besten)
| Code | Regel | Herkunft | Fix | Stufe |
|---|---|---|---|---|
| S001 ★ | Fehlende Klammern bei `if`/`for`/`while` | CS NeedBraces, PMD ControlStatementBraces, EC „Use blocks“ | ✅ | 1 |
| S002 ★ | `if (c) return true; else return false;` | CS SimplifyBooleanReturn, PMD SimplifyBooleanReturns | ✅ | 1 |
| S003 ★ | `x == true`, `!false`, `c ? true : x` | CS SimplifyBooleanExpression, PMD SimplifiedTernary | ✅ | 1 |
| S004 | Zusammenfassbare `if` | PMD CollapsibleIfStatements | ✅ | 1 |
| S005 | Lokale Variable nur für `return` | PMD UnnecessaryLocalBeforeReturn | ✅ | 1 |
| S006 ★ | Überflüssige Klammern | CS UnnecessaryParentheses, PMD UselessParentheses, EP UnnecessaryParentheses, EC „Remove unnecessary parentheses“ | ✅ | 1 |
| S007 ★ | Überflüssige Semikolons (Typ, Member, Enum, try-Ressource), leere Anweisung | CS UnnecessarySemicolon*, EmptyStatement, PMD UnnecessarySemicolon, EC | ✅ | 1 |
| S008 ★ | Überflüssige Modifier (`public` im Interface usw.) | CS RedundantModifier, PMD UnnecessaryModifier, EC | ✅ | 1 |
| S009 | Annotationen vor den Modifier-Keywords | CS ModifierOrder (Annotationsteil) | ✅ | 1 |
| S010 | Überflüssiges `return` am Ende | PMD UnnecessaryReturn | ✅ | 1 |
| S011 | Überflüssiges `super()` | CS AvoidNoArgumentSuperConstructorCall, EC | ✅ | 1 |
| S012 | Explizite Default-Initialisierung (`= null`, `= 0`) | CS ExplicitInitialization | ✅ | 1 |
| S013 | Eine Variable pro Deklaration (mehrere Deklarationen auf einer Zeile trennt fmt) | CS MultipleVariableDeclarations, PMD OneDeclarationPerLine | ✅ | 1 |
| S014 | `int a[]` → `int[] a` | CS ArrayTypeStyle | ✅ | 1 |
| S015 | `1l` → `1L`, Hex-/Präfix-Schreibweise | CS UpperEll, HexLiteralCase, NumericalPrefixesInfixesSuffixesCharacterCase, EP LongLiteralLowerCaseSuffix | ✅ | 1 |
| S016 | Trailing Comma in Arrays/Enums (oder Verbot) | CS ArrayTrailingComma / NoArrayTrailingComma / NoEnumTrailingComma | ✅ | 1 |
| S017 | Leere Blöcke, `if (true)` | CS EmptyBlock, PMD EmptyControlStatement, UnconditionalIfStatement | – | 1 |
| S018 | Unnötige verschachtelte Blöcke | CS AvoidNestedBlocks | – | 1 |
| S019 | `new String("x")`, `new Integer(5)` | PMD StringInstantiation, PrimitiveWrapperInstantiation | ✅ | 1 |
| S020 | Überflüssige `null`-Prüfung vor `instanceof` | CS UnnecessaryNullCheckWithInstanceOf | ✅ | 1 |
| S021 | Lambda-Block mit nur `return` | CS ExpressionOverBlockLambda | ✅ | 1 |
| ~~S022~~ | nicht umgesetzt: Überflüssiges `this.` | CS RedundantThis, EC „Use this qualifier“ | – | – |

### M: Modernisierung (Eclipse-Save-Actions, abhängig von der Java-Zielversion)
| Code | Regel | Ab Java | Herkunft | Fix | Stufe |
|---|---|---|---|---|---|
| M001 ★ | Pattern Matching für `instanceof` | 16 | EC | ✅ | 1 |
| M002 ★ | Switch-Ausdruck / Pfeil-Syntax | 14 | CS UseEnhancedSwitch, EC | ⚠️ | 2 |
| M003 | `when`-Guard statt `if` im `case` | 21 | CS WhenShouldBeUsed | ✅ | 1 |
| M004 ★ | Diamond-Operator (einfache Fälle) | 7 | PMD UseDiamondOperator, EC „Remove redundant type arguments“ | ✅ | 1 |
| M005 | Unnötige Typargumente im Record-Pattern | 21 | CS UnnecessaryTypeArgumentsWithRecordPattern | ✅ | 1 |
| M006 ★ | `var` (nur sichere Fälle) / kein `var` (wählbar) | 10 | EC „Use var“ | ✅ | 1 |
| M007 ★ | Enhanced for über Arrays | 5 | PMD ForLoopCanBeForeach, EC | ✅ | 2 |
| M008 | Unbenutzte catch-/Lambda-/try-Variable → `_` | 22 | CS Unused*ShouldBeUnnamed | ✅ | 2 |
| M009 ★ | Text Block statt verketteter Strings | 15 | EC „Use text blocks“ | ✅ | 1 |
| M010 ★ | Identische catch-Blöcke zusammenfassen | 7 | PMD IdenticalCatchBranches, EC | ⚠️ | 1 |
| M011 | `final` für Parameter | – | CS FinalParameters, EC | ✅ | 1 |
| M012 | Privates Feld kann `final` sein | – | PMD ImmutableField, EC „final for private fields“ | ✅ | 2 |
| M013 | `@Deprecated` passend zu `@deprecated` | – | CS MissingDeprecated, EP DepAnn, EC „Add missing @Deprecated“ | ✅ | 1 |
| M014 | `sealed` ohne `permits` | 17 | CS SealedShouldHavePermitsList | – | 2 |
| M015 | `final` für lokale Variablen mit Wert, die nie neu zugewiesen werden | – | CS FinalLocalVariable (Teil), EC | ✅ | 2 |
| M016 | Überflüssiges `permits` (Gegenregel zu M014) | 17 | CS UnnecessaryPermitsClause | ✅ | 2 |

### N: Namen (Regex konfigurierbar, Property-Namen wie in Checkstyle)
| Code | Regel | Herkunft |
|---|---|---|
| N001 ★ – N012 | Typ, Methode, Instanzfeld, Konstante, statisches Feld, lokale Variable, Parameter (auch Lambda), catch-Parameter, Pattern-Variable, Record-Komponente, Typparameter, Package | CS `*Name`, PMD `*NamingConventions` |
| N013 | Abkürzungen in Namen | CS AbbreviationAsWordInName |
| N014 | Unzulässige Bezeichner (`var`, `record`, `_` …) | CS IllegalIdentifierName |

### A: Design und Aufbau
| Code | Regel | Herkunft | Fix | Stufe |
|---|---|---|---|---|
| A001 | Utility-Klasse ohne privaten Konstruktor | CS HideUtilityClassConstructor, PMD InstantiableUtilityClass | – | 2 |
| A002 | Klasse mit nur privaten Konstruktoren nicht `final` | CS FinalClass, PMD ClassWithOnlyPrivateConstructorsShouldBeFinal | ✅ | 2 |
| A003 | Öffentliche, nicht-finale Felder | CS VisibilityModifier | – | 1 |
| A004 | Veränderliche Exception-Felder | CS MutableException | – | 1 |
| A005 | Interface nur mit Konstanten | CS InterfaceIsType | – | 1 |
| A006 | Mehrere Top-Level-Typen pro Datei | CS OneTopLevelClass, EP MultipleTopLevelClasses | – | 1 |
| A007 | Package passt nicht zum Verzeichnis | CS PackageDeclaration | – | 1 |
| A008 | Überladungen nicht gruppiert, Konstruktoren nicht gruppiert | CS OverloadMethodsDeclarationOrder, ConstructorsDeclarationGrouping, EP UngroupedOverloads | – | 1 |
| ~~A009~~ | nicht umgesetzt: Reihenfolge der Deklarationen | CS DeclarationOrder, EC „Sort members“ | – | – |
| A010 | `throws Exception` in der Signatur | PMD SignatureDeclareThrowsException | – | 1 |
| A011 | Abstrakte Klasse ohne abstrakte Methode | PMD AbstractClassWithoutAbstractMethod | – | 1 |
| A012 | Innere Klasse kann `static` sein | EP ClassCanBeStatic | ✅ | 2 |
| ~~A013~~ | nicht umgesetzt: `main`-Methode übrig geblieben | CS UncommentedMain | – | – |
| A014 | Variable weit vor ihrer Verwendung deklariert | CS VariableDeclarationUsageDistance | – | 2 |
| A015 | Magic Numbers | CS MagicNumber | – | 1 |
| ~~A016~~ | nicht umgesetzt: Doppelte String-Literale | CS MultipleStringLiterals, PMD AvoidDuplicateLiterals | – | – |
| A017 | Dateiname passt nicht zum äußeren Typ | CS OuterTypeFilename | – | 1 |

### C: Komplexität und Größe (Stufe 3, kein Fix, Schwellwerte wie in Checkstyle, in `jtk.toml` einstellbar)
C001 CyclomaticComplexity, C003 BooleanExpressionComplexity, C004 JavaNCSS, C005 ExecutableStatementCount,
C006 MethodLength, C007 FileLength, C008 LambdaBodyLength, C009 AnonInnerLength, C010 ParameterNumber,
C011 RecordComponentNumber, C013 OuterTypeNumber, C014–C016 NestedIf/For/TryDepth, C021 ThrowsCount,
C022 CognitiveComplexity (PMD). Nicht umgesetzt: C002 NPathComplexity, C012 MethodCount, C017 ReturnCount,
C018 ClassFanOutComplexity, C019 ClassDataAbstractionCoupling. Entfernt: C020 LineLength, weil fmt die Zeilenlänge
bestimmt (jtk geht davon aus, dass immer formatiert und gelintet wird).

### D: Javadoc-Inhalt (Stufe 3; das Layout übernimmt die optionale Javadoc-Formatierung in fmt)
MissingJavadocType/Method, JavadocMethod (`@param`/`@return`/`@throws` passend zur Signatur, EP InvalidParam/InvalidThrows),
JavadocType, JavadocVariable, SummaryJavadoc (EP MissingSummary), NonEmptyAtclauseDescription (EP EmptyBlockTag),
AtclauseOrder (✅ Fix), JavadocParamOrder, JavadocThrowsOrder, InvalidJavadocPosition (EP AlmostJavadoc),
JavadocBlockTagLocation, IllegalBlockTag, InappropriateJavadocBlockTagsOn*, JavadocNoErrorInThrowsTag,
PreferCodeOrSnippetJavadocInlineTag, PreferLiteralJavadocInlineTag, EP InvalidInlineTag.

### X: Benutzerdefiniert (Stufe 3)
Header/RegexpHeader, Regexp/RegexpSingleline/RegexpMultiline, RegexpOnFilename, TodoComment, TrailingComment,
AvoidEscapedUnicodeCharacters, IllegalToken/IllegalTokenText, IllegalType, IllegalInstantiation, MatchXpath (als
Abfragesprache auf dem eigenen AST, eher später).

**Nicht vergeben:** B029, S022, A009, A013, A016, C002, C012, C017, C018 und C019 (verworfene Regeln, selten
genutzt oder umstritten). Ihre Codes werden nicht neu vergeben. A016 (doppelte String-Literale) etwa meldet vor allem
gewollte Wiederholungen wie Query-Parameter-, JSON- und Header-Namen, deren Auslagerung in Konstanten den Code eher
schwerer lesbar macht; die Schwellen sind willkürlich (Checkstyle ab 2, Sonar ab 3, PMD ab 4 Vorkommen), und einen
sicheren Fix gibt es nicht, weil Name und Ort der Konstante geraten werden müssten.

## 6. Eclipse-Save-Actions: Zuordnung

Die vollständige Zuordnung aller Eclipse-Clean-Ups (`CleanUpConstants`, 156 Einträge) steht in
[regel-erweiterung.md](regel-erweiterung.md), Abschnitt 5.

| Save Action | Ziel |
|---|---|
| Format source code, Correct indentation, Remove trailing whitespace | fmt ✔ |
| Organize imports (sortieren, Unbenutzte entfernen) | fmt ✔ (fehlende Imports ergänzen: nicht möglich) |
| Use blocks in if/while/for/do | lint S001 |
| Remove unnecessary parentheses | lint S006 |
| Remove redundant semicolons, modifiers, `super()` | lint S007, S008, S011 |
| Remove redundant type arguments | lint M004 |
| Use pattern matching for instanceof | lint M001 |
| Convert to switch expression | lint M002 (⚠️, Pfeil-Form für `switch`-Anweisungen) |
| Use text blocks | lint M009 (✅) |
| Use var | lint M006 (nur sichere Fälle) |
| Convert to enhanced for loop | lint M007 (Arrays; `List` nur mit bekanntem Typargument) |
| Add final modifier (private fields / parameters / locals) | lint M012, M011, M015 |
| Use 'this' qualifier (always / only if necessary) | – (S022 nicht umgesetzt) |
| Add missing @Deprecated | lint M013 |
| Remove unused private members / local variables | lint B026, B027 (⚠️) |
| Combine catch blocks (Quick Fix) | lint M010 (⚠️) |
| Sort members | – (A009 nicht umgesetzt) |
| Add missing @Override, Remove unnecessary casts, Convert to lambda, Add serial version ID, Qualify static access, Remove unnecessary @SuppressWarnings, Unnecessary array creation (varargs) | **nicht umsetzbar ohne Typinformationen** → IDE / OpenRewrite / Error Prone |
| Remove unnecessary `$NON-NLS$` tags | nicht übernehmen (Eclipse-spezifisch) |

**Stand 19.11.2026:** Alle umsetzbaren Save-Actions sind umgesetzt. Fixes, die das Verhalten in Randfällen ändern
können (⚠️), sind *unsafe* und werden wie bei ruff nur mit `--fix --unsafe-fixes` angewandt: B007 (`"a".equals(s)`),
B008 (Literal zuerst), B009 (`x == null`), B026/B027 (Entfernen; B026 nur ohne spätere Zuweisung und mit
seiteneffektfreiem Wert, B027 mit Javadoc, nicht bei überladenen Methoden wegen JUnits `@MethodSource`), M002
(Pfeil-Form) und M010 (Multi-Catch; Untertypen machen ihn ungültig). M006, M007, M009 und M012 haben sichere Fixes
(M009 seit 25.11.2026: String-Literale enthalten keine echten Zeilenumbrüche, und die Formatter lassen Text-Blöcke
unverändert). Abgleich mit PMD auf dessen Regel-Tests: M007 33/33 Zeilen, M010 18/18, M012 171 von 177 (jtk folgt
keinen Zuweisungen in `if`/`else`-Zweigen; dafür erkennt es Zuweisungen über `this(...)`), auf commons-lang und dem
Checkstyle-Quellcode identisch. Alle Fixes zusammen (safe und unsafe) lassen commons-lang kompilierbar (javac,
`src/main`) und vom Formatter akzeptiert; auf dem Checkstyle-Quellcode entstehen keine Syntaxfehler.

## 7. Empfohlenes Standard-Regelset (★)

Ziel: nach dem Umstieg kaum Fehlalarme, viele automatische Fixes. Etwa 25 Regeln:
B001–B004, B007, B009–B011, B014–B017, B019, B026, B027 · S001–S003, S006–S008 · M001, M004 · N001–N012.
Alles andere wird pro Projekt eingeschaltet, idealerweise automatisch aus einer importierten `checkstyle.xml`,
einem PMD-Ruleset oder `org.eclipse.jdt.ui.prefs`.

**Stand 17.11.2026:** Das Standard-Regelset ist umgesetzt (`src/lint/`). Auf den Checkstyle-Testdateien melden
die Regeln mit Checkstyle-Gegenstück 97,5 % der Checkstyle-Verstöße auf derselben Zeile
(`scripts/compare-with-checkstyle.sh`). Die Abweichungen sind dort in den Modulkommentaren beschrieben; die
größten: B027 prüft auch private Methoden, B026 und B027 erkennen Namen in inneren Gültigkeitsbereichen nur nach
dem Namen, und jtk beachtet `@SuppressWarnings`.

**Stand 18.11.2026:** Alle Regeln der Gruppen I, B, S, M, N, A und C mit Checkstyle-Gegenstück sind umgesetzt, mit
Checkstyles Standard-Eigenschaften. Sie sind nicht im Standard-Set; `--select ALL` oder `--extend-select` schaltet sie
ein. Auf den Checkstyle-Testdateien melden die neuen Regeln 99,5 % der Checkstyle-Zeilen (36351 von 36519); fast
alle fehlenden sind Keyword-Reihenfolge-Meldungen von ModifierOrder, die zu fmt gehören (S009 prüft nur Annotationen).
Auf commons-lang und dem Checkstyle-Quellcode stimmen die Gruppen I, B, A und C zeilengenau überein. Noch nicht
umgesetzt, weil sie eine Konfiguration brauchen: I004 (ImportControl), S016 (Trailing Comma, Richtung wählbar),
RequireThis, die Gegenrichtung von M006 (kein `var`), die Gruppen D (Javadoc) und X
(benutzerdefiniert).

**Abgleich mit PMD (18.11.2026):** `scripts/compare-with-pmd.sh` vergleicht die Regeln mit ihren PMD-7.29-Gegenstücken
auf den 5255 Code-Beispielen aus PMDs Regel-Tests. Wo jtk Checkstyle folgt (z. B. B021, B024, M011, M015), ist die
Übereinstimmung gering, weil PMD andere Grenzen zieht. Die Regeln nach PMD stimmen weitgehend überein:
B016 34/34, B017 11/11, B018 6/6, B022 609/609, B028 270/270, A010 79/79, A011 61/61, C022 19/19, S004 41/41,
S005 52/54, S010 45/45, S019 85/88, M004 87/129 (ohne anonyme Klassen und Methodenargumente, die ohne Typen nicht
sicher sind). B019 findet dieselben 7 Fälle, meldet aber an der äußeren `if`-Prüfung statt an der Methode.

**Konfiguration:** `jtk.toml` (die nächste im Verzeichnis der Datei oder darüber) legt Regelauswahl, Java-Version,
`exclude`, `per-file-ignores`, die `[format]`-Schritte und die Parameter der Regeln I002, B020, B021, N013, A014,
A015 und der Gruppe C fest, unter Checkstyles Property-Namen in kebab-case (README, Abschnitt Configuration). Offen: der
Import aus `checkstyle.xml`, PMD-Rulesets und Eclipse-Einstellungen, die Namensmuster von N001–N012.

**Stand 25.11.2026:** Das Standard-Set enthält zusätzlich 66 Regeln aus weiteren Tools (Error Prone, PMD, IntelliJ,
NetBeans, Picnic, Baseline), die ★-Kandidaten aus [regel-erweiterung.md](regel-erweiterung.md) außer S023, P016 und
M041. Außerdem gehören die Save Actions M002, M006, M007, M009 und M010 zum Standard-Set, und `jtk` ohne Befehl
wendet die sicheren Fixes an, formatiert und meldet den Rest (der reine Formatter ist `jtk format`).

## 8. Config-Import: Checkstyle-Module, die nicht nach lint gehören

| Behandlung | Module |
|---|---|
| **Abgedeckt durch fmt**: beim Import mit Hinweis überspringen | alle `whitespace/*` außer den unten genannten, LeftCurly, RightCurly, GoogleRightCurly, Indentation, CommentsIndentation, OperatorWrap, ImportOrder, CustomImportOrder, UnusedImports, RedundantImport, ModifierOrder (Keyword-Teil), OneStatementPerLine, FileTabCharacter, NewlineAtEndOfFile, OpenjdkMethodParameterAlignment, ClassHeaderWrapOpenjdk, AnnotationLocation, JavadocLeadingAsteriskAlign, JavadocMissingLeadingAsterisk |
| **Inkompatibel mit Palantir**: Warnung, Regel deaktivieren | TypeBodyPadding, EmptyLineWrappingInBlock, WhitespaceAround ohne `allowEmpty*`, SeparatorWrap mit `option=eol`, OpenjdkMethodThrowsAlignment, AnnotationOnSameLine, EmptyForIteratorPad, Indentation mit anderen Einrückungswerten, LineLength < 120 |
| **Durch die Javadoc-Option in fmt abgedeckt** (solange sie fehlt: lint ohne Fix) | JavadocParagraph, JavadocTagContinuationIndentation, JavadocMissingWhitespaceAfterAsterisk, SingleLineJavadoc, RequireEmptyLineBeforeBlockTagGroup, JavadocContentLocation, JavadocEndCommentDelimiter, BlockCommentEndPosition, MultilineCommentLeadingAsteriskPresence |
| Nicht übernehmen | Translation, UniqueProperties, OrderedProperties (Properties-Dateien), SuppressWarningsHolder (stattdessen eigene Unterdrückungs-Logik, die `@SuppressWarnings("checkstyle:…")`, `// CHECKSTYLE:OFF`, `// NOPMD` und `suppressions.xml` versteht) |

## 9. Offene Punkte
- **Weitere Regelquellen:** [regel-erweiterung.md](regel-erweiterung.md) wertet IntelliJ, Error Prone, PMD, NetBeans,
  Picnic, Palantir Baseline, Modernizer, forbidden-apis, javac und Eclipse JDT vollständig aus und schlägt die
  geeigneten Regeln als distinktes Regelset mit Codes vor.
- **CLI:** Die heutige CLI ist kompatibel zur Original-CLI (`jtk [optionen] dateien`). Vorschlag: `jtk fmt …` mit
  denselben Optionen, `jtk lint …` neu; der Aufruf ohne Unterbefehl bleibt als Alias für `fmt`.
- **Regelnamen prüfen:** Die PMD-Namen sind gegen den Regelkatalog von PMD 7.29.0 geprüft (18.11.2026; korrigiert:
  UseUtilityClass heißt jetzt InstantiableUtilityClass, MissingBreakInSwitch gibt es nicht mehr). Die
  Error-Prone-Namen stammen noch nicht aus dessen Repo. Die Checkstyle-Namen stammen aus dem geklonten Repo.
- **Messung reproduzieren:** Skripte unter `scratchpad/` (`genconfig.py`, `prep.py`, `table.py`); bei Bedarf als
  `scripts/checkstyle-compare.sh` ins Repo übernehmen.
