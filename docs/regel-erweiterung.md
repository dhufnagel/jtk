# Regelkatalog: Erweiterung von `jtk lint` aus weiteren Open-Source-Tools

Stand: 24.11.2026 · Ergänzt [regel-analyse.md](regel-analyse.md), Abschnitt 5

## 1. Quellen und Vorgehen

Ausgewertet wurden die vollständigen Regelkataloge der folgenden Tools (jeweils aktueller `main`-Stand):

| Tool | Lizenz | Ausgewertet | Einträge |
|---|---|---|---:|
| IntelliJ IDEA Community | Apache 2.0 | Java-Inspektionen (`java/`, `jvm/`, `plugins/junit`) | 966 |
| Error Prone | Apache 2.0 | alle `@BugPattern` in `core` | 653 |
| PMD 7 | BSD-artig | alle Java-Regeln (`category/java/*.xml`) | 313 |
| Apache NetBeans java.hints | Apache 2.0 | `@Hint`-Klassen und `.hint`-Dateien | 222 |
| Picnic Error Prone Support | MIT | Bugpatterns und Refaster-Regeln (gruppiert) | 203 |
| Palantir baseline-error-prone | Apache 2.0 | alle `@BugPattern` | 114 |
| Modernizer Maven Plugin | Apache 2.0 | `modernizer.xml`, nach Thema gruppiert | 31 |
| forbidden-apis | Apache 2.0 | mitgelieferte Signaturlisten, nach Familie | 7 |
| javac `-Xlint` | GPLv2 + CPE | Lint-Kategorien und einzelne Warnungen | 66 |
| Eclipse JDT Compiler | EPL-2.0 | `JavaCore.COMPILER_PB_*`-Optionen | 127 |
| Eclipse JDT Clean-Ups | EPL-2.0 | `CleanUpConstants` (Clean-Ups und Save-Actions), Abschnitt 5 | 156 |

Checkstyle ist bereits vollständig ausgewertet (regel-analyse.md). PMD und Error Prone waren nur teilweise übernommen und sind hier vollständig enthalten. SonarJava (inzwischen Sonar Source-Available License), Teile von OpenRewrite (Moderne Source Available License) und Semgrep-Regeln (Semgrep Rules License) sind nicht Open Source im OSI-Sinn und deshalb nicht enthalten; SpotBugs, Infer, NullAway und das Checker Framework arbeiten auf Bytecode oder mit Typ- und Datenflussanalyse und passen nicht zum Ansatz von jtk.

Vorgehen:

1. **Klassifizieren** (2702 Einträge): Jede Regel wurde als geeignet (Stufe 1, 2 oder 2*) oder ungeeignet (mit Grund) eingestuft und gegen die bestehenden jtk-Regeln abgeglichen.
2. **Deduplizieren**: Regeln verschiedener Tools, die dasselbe Problem melden, sind eine Regel; Gegenregeln sind eine Regel mit wählbarer Richtung. Danach ein zweiter Abgleich über die Kategoriegrenzen hinweg.
3. **Codes vergeben**: je Kategorie fortlaufend nach den bestehenden Codes, thematisch sortiert. Die verworfenen Codes (B029, S022, A009, A013, A016, C002, C012, C017–C020) bleiben frei.

Die Einstufung ist eine Schätzung aus den Regelbeschreibungen, nicht aus einer Implementierung; vor dem Umsetzen einer Regel lohnt der Blick in die Quelle und die Testfälle des Originals.

## 2. Ergebnis

**837 neue, distinkte Regeln**, davon 329 auf Stufe 1, 283 auf Stufe 2 und 225 auf Stufe 2*; 287 mit sicherem Fix, 166 mit unsicherem Fix; 69 Kandidaten für das Standard-Regelset (★). Dazu kommen 416 Regeln der Tools, die bestehende jtk-Regeln schon abdecken (Abschnitt 4), und 861 ungeeignete (Abschnitt 6). Abschnitt 5 ordnet alle Eclipse-Clean-Ups und Save-Actions zu.

| Kategorie | Neue Regeln | Codes | ★ |
|---|---:|---|---:|
| B Fehlerquellen | 250 | B035–B284 | 19 |
| E Exceptions und Ressourcen (neu) | 15 | E001–E015 | 0 |
| S Vereinfachungen | 165 | S023–S187 | 12 |
| M Modernisierung | 45 | M017–M061 | 2 |
| P Performance (neu) | 49 | P001–P049 | 1 |
| A Design und Aufbau | 91 | A018–A108 | 2 |
| N Namen | 24 | N015–N038 | 1 |
| C Komplexität und Größe | 20 | C023–C042 | 0 |
| I Imports | 4 | I006–I009 | 0 |
| D Javadoc-Inhalt | 14 | D001–D014 | 0 |
| X Benutzerdefiniert | 4 | X001–X004 | 0 |
| T Tests (neu) | 67 | T001–T067 | 13 |
| L Logging (neu) | 11 | L001–L011 | 4 |
| K Nebenläufigkeit (neu) | 64 | K001–K064 | 13 |
| V Sicherheit (neu) | 14 | V001–V014 | 2 |
| **Summe** | **837** | | **69** |

Regeln je Quelle (eine Regel zählt bei jedem Tool, aus dem sie stammt): IntelliJ IDEA Community 490, Error Prone 323, PMD 7 144, Apache NetBeans java.hints 92, Picnic Error Prone Support 79, Palantir baseline-error-prone 52, Modernizer Maven Plugin 19, forbidden-apis 4, javac `-Xlint` 20, Eclipse JDT Compiler 21, Eclipse JDT Clean-Ups 11.

Legende: **Stufe** 1 = rein syntaktisch, 2 = Symboltabelle der Datei bzw. Import-Auflösung, 2* = nur für Ausdrücke mit lokal bekanntem Typ (Literal, `new X()`, lokal deklarierte Variable, Feld, Parameter), also mit geringerer Trefferquote als im Original, aber ohne Fehlalarme. **Fix** ✅ sicher, ⚠️ unsicher (nur mit `--unsafe-fixes`), – keiner. **★** Kandidat für das Standard-Regelset. **Herkunft** nennt alle Tools und Regelnamen, aus denen die Regel zusammengeführt ist.

Neue Kategorien: **E** Exception- und Ressourcen-Design, **P** Performance, **T** Tests (JUnit 4/5, TestNG, AssertJ, Hamcrest, Mockito), **L** Logging (SLF4J, Log4j 2, java.util.logging), **K** Nebenläufigkeit, **V** Sicherheit.

## 3. Regeln

### B: Fehlerquellen

| Code | Regel | Stufe | Fix | Herkunft | Beispiel | Hinweis |
|---|---|---|---|---|---|---|
| B035 | equals ruft sich selbst rekursiv auf ★ | 1 | – | errorprone:EqualsReference | `boolean equals(Object o) { return equals(o); }` |  |
| B036 | equals castet ohne Typprüfung des Parameters | 1 | – | errorprone:EqualsUnsafeCast, intellij:EqualsWhichDoesntCheckParameterClass, netbeans:.equals Method not Checking Type (general) | `public boolean equals(Object o) { return id == ((Foo) o).id; }` |  |
| B037 | equals vergleicht nur hashCode | 1 | – | errorprone:EqualsUsingHashCode | `return hashCode() == o.hashCode();` |  |
| B038 | equals vergleicht unterschiedliche Felder | 1 | – | errorprone:EqualsWrongThing | `a == that.a && b == that.a` |  |
| B039 | super.equals/hashCode ruft nur die Object-Implementierung | 1 | ⚠️ | errorprone:SuperCallToObjectMethod | `class A { boolean equals(Object o) { return super.equals(o) && … } }` |  |
| B040 | DoubleMath.fuzzyEquals in equals | 2 | – | errorprone:FuzzyEqualsShouldNotBeUsedInEqualsMethod | `boolean equals(Object o) { … fuzzyEquals(a, b, e) … }` | Guava |
| B041 | hashCode nutzt Felder, die equals nicht vergleicht | 2 | – | errorprone:InconsistentHashCode | `equals vergleicht a, hashCode nutzt a und b` |  |
| B042 | equals/hashCode/compareTo liest nicht-finales Feld | 2 | – | intellij:EqualsUsesNonFinalVariable, intellij:HashCodeUsesNonFinalVariable, intellij:CompareToUsesNonFinalVariable | `int hashCode() { return name.hashCode(); } mit nicht finalem name` |  |
| B043 | equals/hashCode-Methode falsch benannt oder mit falschen Parametern | 1 | ⚠️ | intellij:MisspelledEquals, pmd:SuspiciousEqualsMethodName, pmd:SuspiciousHashcodeMethodName | `public boolean equal(Object o)` → `equals; int hashcode()` → `hashCode()` |  |
| B044 | equals mit sich selbst aufgerufen | 1 | – | intellij:EqualsWithItself | `x.equals(x)` | Ergänzt B011 |
| B045 | equals mit unverträglichen Typen | 2* | – | errorprone:EqualsIncompatibleType, intellij:EqualsBetweenInconvertibleTypes, eclipse:COMPILER_PB_UNLIKELY_EQUALS_ARGUMENT_TYPE | `"a".equals(5)` |  |
| B046 | equals auf Typ ohne inhaltliches equals (StringBuilder, Iterable) | 2* | – | intellij:EqualsOnSuspiciousObject, errorprone:UndefinedEquals | `StringBuilder a, b; a.equals(b)` |  |
| B047 | String.equals mit CharSequence-Argument | 2* | ⚠️ | intellij:StringEqualsCharSequence | `s.equals(sb)` → `s.contentEquals(sb)` |  |
| B048 | java.net.URL in equals/hashCode oder Hash-Collection | 2* | – | intellij:UrlHashCode, errorprone:URLEqualsHashCode | `Set<URL> s = new HashSet<>(); url.equals(other)` |  |
| B049 | Comparable mit fremdem Typargument | 1 | – | errorprone:ComparableType | `class A implements Comparable<B>` |  |
| B050 | Comparable ohne equals | 1 | – | intellij:ComparableImplementedButEqualsNotOverridden, pmd:OverrideBothEqualsAndHashCodeOnComparable | `class A implements Comparable<A> { int compareTo(A o) {…} }` |  |
| B051 | compare/compareTo ignoriert einen Parameter | 1 | – | netbeans:Comparator.compare() does not use its parameter {0} (bugs), intellij:ComparatorMethodParameterNotUsed | `int compare(A a, A b) { return a.x - a.y; }` |  |
| B052 | Differenz als Ergebnis von compare/compareTo (Überlauf) | 1 | ⚠️ | intellij:SubtractionInCompareTo, errorprone:BadComparable | `return a - b;` → `return Integer.compare(a, b);` |  |
| B053 | Vergleichsmethode liefert nie 0 | 1 | – | errorprone:ComparisonContractViolated | `return a > b ? 1 : -1;` |  |
| B054 | compareTo-Ergebnis mit 1/-1 verglichen ★ | 1 | ✅ | errorprone:CompareToZero, intellij:ComparatorResultComparison | `a.compareTo(b) == 1` → `a.compareTo(b) > 0` |  |
| B055 | Methodenreferenz als Comparator verletzt den Vertrag | 2* | ⚠️ | intellij:InvalidComparatorMethodReference | `list.sort(Integer::max)` → `list.sort(Integer::compare)` |  |
| B056 | Comparator nicht Serializable | 1 | – | intellij:ComparatorNotSerializable | `class ByName implements Comparator<P>` |  |
| B057 | Iterator.hasNext ruft next() auf | 2 | – | intellij:IteratorHasNextCallsIteratorNext | `public boolean hasNext() { return next() != null; }` |  |
| B058 | Iterator.next wirft nie NoSuchElementException | 2 | – | intellij:IteratorNextDoesNotThrowNoSuchElementException | `public T next() { return items[i++]; }` |  |
| B059 | toString gibt null zurück | 1 | – | errorprone:ToStringReturnsNull | `public String toString() { return null; }` |  |
| B060 | self()/getThis() gibt nicht this zurück | 1 | ✅ | errorprone:SelfAlwaysReturnsThis | `Foo self() { return new Foo(); }` → `return this;` |  |
| B061 | Getter/Setter greift auf unerwartetes Feld zu | 2 | – | intellij:SuspiciousGetterSetter | `int getY() { return x; }` |  |
| B062 | Kopierkonstruktor kopiert nicht alle Felder | 2 | – | intellij:CopyConstructorMissesField | `Foo(Foo o) { this.a = o.a; } // b fehlt` |  |
| B063 | Konstruktorverkettung ignoriert gleichnamigen Parameter | 2 | – | errorprone:ChainingConstructorIgnoresParameter | `Foo(int a, int b) { this(a, 0); }` |  |
| B064 | InvocationHandler behandelt equals/hashCode/toString nicht | 2 | – | intellij:SuspiciousInvocationHandlerImplementation | `invoke(…) { return m.invoke(t, a); }` |  |
| B065 | InvocationHandler packt InvocationTargetException nicht aus | 2* | – | baseline:InvocationHandlerDelegation | `method.invoke(delegate, args) ohne catch (InvocationTargetException)` |  |
| B066 | equals/Objects.equals auf Arrays vergleicht nur Referenzen | 2* | ⚠️ | intellij:ArrayEquals, netbeans:.equals on Array (bugs), errorprone:ArrayEquals, intellij:ArrayObjectsEquals | `a.equals(b)` → `Arrays.equals(a, b)` |  |
| B067 | Arrays mit == verglichen | 2* | ⚠️ | intellij:ArrayEquality | `int[] a, b; a == b` → `Arrays.equals(a, b)` |  |
| B068 | hashCode auf Array | 2* | ⚠️ | errorprone:ArrayHashCode, intellij:ArrayHashCode, netbeans:hashCode() used on array instance (bugs) | `arr.hashCode()` → `Arrays.hashCode(arr)` |  |
| B069 | Array in String umgewandelt (toString, Verkettung) | 2* | ⚠️ | errorprone:ArrayToString, netbeans:toString() used on array instance (bugs), intellij:ImplicitArrayToString, eclipse:COMPILER_PB_CHAR_ARRAY_IN_STRING_CONCATENATION | `"x" + arr, arr.toString()` → `Arrays.toString(arr)` | char[]: String.valueOf |
| B070 | Verdächtiger Array-Downcast | 2* | – | intellij:SuspiciousArrayCast | `Object[] a = new Object[2]; String[] s = (String[]) a;` |  |
| B071 | Cast des Ergebnisses von toArray() | 1 | ✅ | pmd:ClassCastExceptionWithToArray | `(String[]) list.toArray()` → `list.toArray(new String[0])` |  |
| B072 | toArray mit unpassendem Array-Typ | 2* | – | intellij:SuspiciousToArrayCall | `List<String> l; l.toArray(new Integer[0])` |  |
| B073 | Verdächtige System.arraycopy-Argumente | 2* | – | netbeans:Suspicious invocation of System.arraycopy (bugs), intellij:SuspiciousSystemArraycopy | `System.arraycopy(src, -1, dst, 0, n)` |  |
| B074 | Primitives Array an Object-Varargs übergeben | 2* | – | errorprone:ArraysAsListPrimitiveArray, errorprone:PrimitiveArrayPassedToVarargsMethod, intellij:PrimitiveArrayArgumentToVariableArgMethod, netbeans:Confusing primitive array passed to varargs method (bugs) | `int[] a; Arrays.asList(a)` |  |
| B075 | Collection als einzelnes Varargs-Element übergeben | 2* | – | intellij:IterableUsedAsVararg | `List<String> l; Arrays.asList(l)` |  |
| B076 | null als einziges Varargs-Argument | 2* | ✅ | errorprone:NullNeedsCastForVarargs, intellij:NullArgumentToVariableArgMethod, netbeans:Confusing null passed to varargs method (bugs) | `Arrays.asList(null)` → `Arrays.asList((Object) null)` |  |
| B077 | Collection/Array nur befüllt und nie gelesen | 2 | – | errorprone:ModifiedButNotUsed, intellij:MismatchedCollectionQueryUpdate, netbeans:Unbalanced read/write with collections (bugs), intellij:MismatchedArrayReadWrite, netbeans:Unbalanced read/write with arrays (bugs), intellij:MismatchedStringBuilderQueryUpdate | `List<A> l = new ArrayList<>(); l.add(a); (nie gelesen)` | auch StringBuilder |
| B078 | Vergleich mit NaN per == ★ | 1 | ✅ | errorprone:EqualsNaN, pmd:ComparisonWithNaN, intellij:ComparisonToNaN | `x == Double.NaN` → `Double.isNaN(x)` |  |
| B079 | Gleitkommazahlen mit == verglichen | 2* | – | intellij:FloatingPointEquality | `double a, b; a == b` |  |
| B080 | Wrapper-Zahlen mit == verglichen | 2* | ⚠️ | netbeans:Boxed value identity comparison (bugs), intellij:NumberEquality, errorprone:BoxedPrimitiveEquality | `Integer a, b; a == b` → `a.equals(b)` |  |
| B081 | Objekte mit == statt equals verglichen | 2* | ⚠️ | pmd:CompareObjectsWithEquals | `String a, b; a == b` → `a.equals(b)` | Obermenge der ==-Regeln; Fehlalarme bei gewollter Identität |
| B082 | Identität wertbasierter Typen verwendet (==, synchronized) | 2* | ⚠️ | errorprone:ValueClassIdentity, errorprone:OptionalEquality, errorprone:MemorySegmentReferenceEquality | `Optional<A> a, b; a == b` → `a.equals(b)` |  |
| B083 | Neu erzeugtes Objekt mit == verglichen | 1 | – | intellij:NewObjectEquality | `x == new Foo()` |  |
| B084 | this mit null verglichen | 1 | ✅ | errorprone:ComparingThisWithNull | `this == null` → `false` |  |
| B085 | Verkettete Gleichheitsvergleiche | 1 | – | intellij:ChainedEquality | `a == b == c` |  |
| B086 | Bitausdruck mit konstantem Ergebnis ★ | 1 | – | netbeans:Incompatible mask (bitwise_operations), intellij:IncompatibleMask, errorprone:ErroneousBitwiseExpression | `(x & 2) == 1; x & 1 & 2` |  |
| B087 | Vergleich ist immer wahr oder falsch (indexOf >= -1, size() >= 0) ★ | 1 | – | intellij:PointlessIndexOfComparison, errorprone:SizeGreaterThanOrEqualsZero | `list.size() >= 0; s.indexOf("a") >= -1` |  |
| B088 | Vergleich mit Wert außerhalb des Wertebereichs | 2* | – | errorprone:ComparisonOutOfRange | `byte b; b == 200` |  |
| B089 | Vergleich von short und char | 2* | – | intellij:ComparisonOfShortAndChar | `short s; char c; s == c` |  |
| B090 | Ungerade-Prüfung mit % 2 == 1 ★ | 1 | ⚠️ | intellij:BadOddness | `x % 2 == 1` → `x % 2 != 0` |  |
| B091 | Zu breite Compare-Methode verliert Genauigkeit | 2* | ✅ | errorprone:LossyPrimitiveCompare | `long a; Float.compare(a, b)` → `Long.compare(a, b)` |  |
| B092 | & bzw. \| statt && bzw. \|\| bei booleans | 2* | ⚠️ | intellij:NonShortCircuitBoolean, errorprone:ShortCircuitBoolean | `a != null & a.ok()` → `a != null && a.ok()` |  |
| B093 | Konstanter Ausdruck läuft über ★ | 1 | ⚠️ | errorprone:ConstantOverflow, intellij:NumericOverflow | `long ms = 365 * 24 * 3600 * 1000;` → `365L * …` |  |
| B094 | int-Ausdruck läuft vor Erweiterung zu long über | 2* | ⚠️ | intellij:IntegerMultiplicationImplicitCastToLong, errorprone:IntLongMath | `long v = 65536 * i;` → `65536L * i` |  |
| B095 | Ganzzahldivision im Gleitkomma-Kontext | 2* | ⚠️ | errorprone:NarrowCalculation, intellij:SuspiciousIntegerDivAssignment, intellij:IntegerDivisionInFloatingPointContext | `double d = 1 / 2;` → `1.0 / 2` |  |
| B096 | Division durch konstante Null ★ | 1 | – | intellij:DivideByZero, javac:divzero | `x / 0` |  |
| B097 | Shift-Weite außerhalb des Wertebereichs | 2* | – | errorprone:BadShiftAmount, netbeans:Shift operation outside of the reasonable range (bitwise_operations), intellij:ShiftOutOfRange, javac:compiler.warn.bit.shift.out.of.range | `int x; x << 32` |  |
| B098 | Cast mit Genauigkeitsverlust | 2* | – | intellij:CastThatLosesPrecision | `long l; (int) l` |  |
| B099 | Verbundzuweisung mit verstecktem verengendem Cast | 2* | – | errorprone:NarrowingCompoundAssignment, intellij:LossyConversionCompoundAssignment, javac:lossy-conversions | `int i; i += 1.5;` |  |
| B100 | Cast-Präzedenz unklar | 1 | ✅ | errorprone:FloatCast | `(int) 0.5 * x` → `((int) 0.5) * x` | Ergänzt B030 |
| B101 | (int) Math.random() ist immer 0 ★ | 1 | – | intellij:MathRandomCastToInt, netbeans:Math.random() casted to int (bugs), errorprone:RandomCast | `(int) Math.random() * 10` → `(int) (Math.random() * 10)` |  |
| B102 | Random.nextDouble() für Zufalls-int | 2* | ⚠️ | intellij:RandomDoubleForRandomInteger | `(int) (rnd.nextDouble() * n)` → `rnd.nextInt(n)` |  |
| B103 | Random.nextInt() % n kann negativ sein | 2* | ⚠️ | errorprone:RandomModInteger | `random.nextInt() % n` → `random.nextInt(n)` |  |
| B104 | Math.abs auf Zufallszahl oder Hash kann negativ sein | 2 | – | errorprone:MathAbsoluteNegative | `Math.abs(random.nextInt())` |  |
| B105 | Math.round/ceil/floor mit Ganzzahl | 2* | ✅ | errorprone:MathRoundIntLong, intellij:MathRoundingWithIntArgument | `int i; Math.round(i)` → `i` |  |
| B106 | Objects.hashCode mit primitivem Wert | 2* | ✅ | errorprone:ObjectsHashCodePrimitive | `int i; Objects.hashCode(i)` → `Integer.hashCode(i)` |  |
| B107 | char in arithmetischem Ausdruck | 2* | – | intellij:CharUsedInArithmeticContext | `char c; int n = c + 1;` |  |
| B108 | ^ als Potenzoperator missverstanden ★ | 1 | ✅ | errorprone:XorPower | `2 ^ 8` → `1 << 8` |  |
| B109 | Oktales Ganzzahl-Literal | 1 | – | pmd:AvoidUsingOctalValues, intellij:OctalLiteral, intellij:OctalAndDecimalIntegersMixed | `int i = 012;` | Ergänzt B018 |
| B110 | Negativer Wert zu char gecastet | 1 | ✅ | errorprone:NegativeCharLiteral | `(char) -1` → `Character.MAX_VALUE` |  |
| B111 | -0 ist kein negatives Null-Gleitkomma | 1 | ✅ | errorprone:AttemptedNegativeZero | `double d = -0;` → `-0.0` |  |
| B112 | Negative int-Hex-Konstante im long-Kontext | 2 | ⚠️ | intellij:NegativeIntConstantInLongContext | `long mask = 0xFFFF_FFFF;` → `0xFFFF_FFFFL` |  |
| B113 | Gleitkomma-Literal verliert Genauigkeit | 1 | ✅ | errorprone:FloatingPointLiteralPrecision | `float f = 1.0000000001f;` |  |
| B114 | Zifferngruppen mit Unterstrich nicht in Dreiergruppen | 1 | – | intellij:SuspiciousLiteralUnderscore | `int n = 1_00_000;` |  |
| B115 | Unäres Plus | 1 | ✅ | intellij:UnaryPlus | `i =+ 1; int x = +y;` |  |
| B116 | Mehrere unäre Operatoren hintereinander | 1 | ✅ | pmd:AvoidMultipleUnaryOperators | `!!x` → `x` |  |
| B117 | Mehrere ++/-- auf dieselbe Variable in einem Ausdruck | 1 | – | errorprone:MultipleUnaryOperatorsInMethodCall | `f(i++, i++)` |  |
| B118 | Wert von ++/-- in Ausdruck verwendet | 1 | – | netbeans:Value of increment/decrement expression used (assignment_issues), intellij:IncrementDecrementUsedAsExpression | `a[i++] = x;` | strenger als B012 |
| B119 | Ergebnis eines Postfix-Operators im Lambda verworfen | 1 | – | errorprone:DiscardedPostfixExpression | `x -> x++` |  |
| B120 | Bedingungsausdruck mit null wird ausgepackt | 2* | – | errorprone:NullTernary | `int x = c ? null : 1;` |  |
| B121 | Numerische Promotion im Bedingungsausdruck | 2* | ⚠️ | errorprone:ConditionalExpressionNumericPromotion | `Object o = c ? 1 : 2L;` |  |
| B122 | BigDecimal.equals vergleicht auch die Skala | 2* | ⚠️ | errorprone:BigDecimalEquals, intellij:BigDecimalEquals | `a.equals(b)` → `a.compareTo(b) == 0` |  |
| B123 | BigDecimal.divide/setScale ohne Rundungsmodus | 2* | – | intellij:BigDecimalMethodWithoutRoundingCalled | `a.divide(b)` → `a.divide(b, RoundingMode.HALF_UP)` |  |
| B124 | Argumente laut Namen vertauscht | 2* | – | errorprone:ArgumentSelectionDefectChecker, intellij:SuspiciousNameCombination | `void f(int width, int height); f(height, width)` |  |
| B125 | /* name= */-Kommentar passt nicht zum Parameter | 2* | ✅ | errorprone:ParameterName | `f(/* width= */ h) bei f(int height)` |  |
| B126 | Argumente von indexOf/nCopies vertauscht | 2* | ✅ | errorprone:IndexOfChar, errorprone:NCopiesOfChar | `s.indexOf(0, 'x')` → `s.indexOf('x', 0)` |  |
| B127 | Ungültiger Formatstring oder falsche Argumentzahl | 2* | – | errorprone:FormatString, intellij:MalformedFormatString, netbeans:Malformed format string (bugs) | `String.format("%d %s", 1)` |  |
| B128 | Formatplatzhalter ohne Formatmethode | 2* | – | errorprone:OrphanedFormatString | `throw new IllegalStateException("bad %s")` |  |
| B129 | Verkettung im Format- oder MessageFormat-Muster | 1 | ⚠️ | intellij:StringConcatenationInFormatCall, intellij:StringConcatenationInMessageFormatCall, errorprone:FormatStringShouldUsePlaceholders | `String.format("Hi " + name + " %d", n)` → `String.format("Hi %s %d", name, n)` |  |
| B130 | Ungültiges MessageFormat-Muster | 2 | – | intellij:IncorrectMessageFormat | `MessageFormat.format("{0", x)` |  |
| B131 | Fehlerhafte Platzhalter in Guava-Preconditions | 2 | ⚠️ | errorprone:LenientFormatStringValidation, errorprone:PreconditionsInvalidPlaceholder, baseline:GuavaPreconditionsMessageFormat | `checkArgument(c, "x {}", y)` → `"x %s"` | Guava |
| B132 | Fehlendes Leerzeichen bei String-Verkettung | 1 | – | intellij:StringConcatenationMissingWhitespace | `"SELECT col" + "FROM t"` |  |
| B133 | Verwirrendes \s-Escape | 1 | ✅ | intellij:EscapedSpace, errorprone:MisleadingEscapedSpace | `"a\sb"` → `"a b"` |  |
| B134 | Text Block mit gemischter Einrückung | 1 | – | intellij:InconsistentTextBlockIndent, javac:compiler.warn.inconsistent.white.space.indentation, javac:text-blocks | `"""\n  a\n\tb"""` |  |
| B135 | Abschließende Leerzeichen im Text Block | 1 | ⚠️ | intellij:TrailingWhitespacesInTextBlock, javac:compiler.warn.trailing.white.space.will.be.removed | `"""\n  abc   \n"""` → `abc\s` |  |
| B136 | String.join ohne Elemente | 2 | – | errorprone:StringJoin | `String.join(",")` |  |
| B137 | replace() auf Literal ohne Wirkung | 1 | ✅ | intellij:ReplaceOnLiteralHasNoEffect | `"abc".replace("x", "y")` → `"abc"` |  |
| B138 | String-Suche mit unpassender Groß-/Kleinschreibung | 2* | – | intellij:MismatchedStringCase | `s.toUpperCase().contains("a")` |  |
| B139 | String.split mit überraschendem Verhalten | 2* | ⚠️ | errorprone:StringSplitter | `s.split(",")` → `Splitter.on(',').split(s)` | Guava |
| B140 | StringBuilder(char) setzt die Kapazität ★ | 1 | ✅ | intellij:NewStringBufferWithCharArgument, errorprone:StringBuilderInitWithChar, netbeans:StringBuffer constructor called with `char'' argument (bugs), pmd:StringBufferInstantiationWithChar | `new StringBuilder('a')` → `new StringBuilder("a")` |  |
| B141 | Doppelte Zeichen im StringTokenizer-Trenner | 2 | ✅ | intellij:StringTokenizerDelimiter | `new StringTokenizer(s, ",,")` → `","` |  |
| B142 | Regex-Metazeichen "." als Muster ★ | 1 | ⚠️ | netbeans:Single Char Regex (bugs), intellij:ReplaceAllDot, errorprone:BareDotMetacharacter | `s.split(".")` → `s.split("\\.")` |  |
| B143 | Ungültiger regulärer Ausdruck ★ | 2 | – | errorprone:InvalidPatternSyntax, netbeans:DN_org.netbeans.modules.java.hints.jdk.Regexp (bugs) | `Pattern.compile("[")` |  |
| B144 | Ungültiger XPath-Ausdruck | 2* | – | netbeans:Malformed XPath expression (bugs) | `xpath.compile("//a[")` |  |
| B145 | Doppelter Schlüssel oder Element (Map.of, Set.of, put) | 2* | – | errorprone:DistinctVarargsChecker, errorprone:DuplicateMapKeys, intellij:OverwrittenKey | `Map.ofEntries(entry("a", 1), entry("a", 2)); map.put("a", 1); map.put("a", 2);` |  |
| B146 | Implizite Default-Locale | 2* | ⚠️ | errorprone:DefaultLocale, pmd:UseLocaleWithCaseConversions, intellij:StringToUpperWithoutLocale, errorprone:StringCaseLocaleUsage, intellij:SimpleDateFormatWithoutLocale, pmd:SimpleDateFormatNeedsLocale, forbidden-apis:jdk-unsafe, forbidden-apis:incr-jdk-unsafe | `s.toLowerCase()` → `s.toLowerCase(Locale.ROOT)` | forbidden-apis jdk-unsafe umfasst auch Charset und Zeitzone |
| B147 | Implizites Default-Charset | 2* | ⚠️ | errorprone:DefaultCharset, intellij:ImplicitDefaultCharsetUsage, pmd:RelianceOnDefaultCharset, forbidden-apis:commons-io-unsafe | `new FileReader(f)` → `new FileReader(f, UTF_8)` |  |
| B148 | Implizite System-Zeitzone oder -Uhr | 2 | ⚠️ | errorprone:JavaTimeDefaultTimeZone, baseline:JavaTimeSystemDefaultTimeZone, picnic:TimeZoneUsage | `LocalDate.now()` → `LocalDate.now(clock)` |  |
| B149 | Unsichere Locale-Erzeugung | 2* | ⚠️ | errorprone:UnsafeLocaleUsage | `new Locale("en_US")` → `Locale.forLanguageTag("en-US")` |  |
| B150 | Verdächtiges Datumsmuster (YYYY, DD) ★ | 2 | ✅ | errorprone:MisusedWeekYear, errorprone:MisusedDayOfYear, intellij:SuspiciousDateFormat | `"YYYY-MM-dd"` → `"yyyy-MM-dd"` |  |
| B151 | Ungültiges Datumsmuster | 2 | – | intellij:IncorrectDateTimeFormat | `DateTimeFormatter.ofPattern("TT")` |  |
| B152 | Feld im Datumsmuster doppelt | 2 | – | errorprone:DuplicateDateFormatField | `new SimpleDateFormat("mm:mm")` |  |
| B153 | Verdächtige java.util.Date-Argumente | 2 | – | errorprone:DateChecker | `new Date(2020, 12, 1)` |  |
| B154 | toString() auf java.util.Date/java.sql.Time | 2* | – | intellij:DateToString, intellij:TimeToString | `Date d; d.toString()` |  |
| B155 | Aktuelle Zeit im statischen Initialisierer | 2 | – | errorprone:TimeInStaticInitializer | `static final Instant START = Instant.now();` |  |
| B156 | java.time-Konstante außerhalb des Wertebereichs | 2 | – | errorprone:InvalidJavaTimeConstant | `LocalDate.of(2020, 13, 1)` |  |
| B157 | java.time mit nicht unterstütztem ChronoField/ChronoUnit | 2* | – | intellij:UnsupportedChronoFieldUnitCall, errorprone:TemporalAccessorGetChronoField, errorprone:DurationGetTemporalUnit, errorprone:PeriodGetTemporalUnit, errorprone:InstantTemporalUnit, errorprone:DurationTemporalUnit | `Duration.of(1, ChronoUnit.MONTHS)` |  |
| B158 | java.time-from mit unpassendem Typ | 2* | – | errorprone:FromTemporalAccessor | `LocalDate.from(instant)` |  |
| B159 | Duration.from/Period.from mit gleichem Typ | 2* | ✅ | errorprone:DurationFrom, errorprone:PeriodFrom | `Duration.from(d)` → `d` |  |
| B160 | Period und Duration vermischt | 2* | – | errorprone:LocalDateTemporalAmount, errorprone:PeriodTimeMath | `date.plus(Duration.ofDays(1))` → `date.plusDays(1)` |  |
| B161 | getNano() ohne getSeconds()/getEpochSecond() | 2* | – | errorprone:JavaDurationGetSecondsGetNano, errorprone:JavaInstantGetSecondsGetNano, errorprone:JavaLocalDateTimeGetNano, errorprone:JavaLocalTimeGetNano | `d.getNano()` |  |
| B162 | Duration.withNanos/withSeconds | 2* | – | errorprone:JavaDurationWithNanos, errorprone:JavaDurationWithSeconds | `d.withSeconds(5)` |  |
| B163 | Period.getDays liefert nur den Tagesanteil | 2* | – | errorprone:JavaPeriodGetDays | `period.getDays()` |  |
| B164 | Verdächtige TimeUnit-Umrechnung | 2 | – | errorprone:TimeUnitConversionChecker | `TimeUnit.MILLISECONDS.toSeconds(10)` |  |
| B165 | Wert in falscher Zeiteinheit laut Namen | 2* | – | errorprone:TimeUnitMismatch | `sleepMillis(timeoutSeconds)` |  |
| B166 | Ungültige Zeitzonen-ID | 2 | – | errorprone:InvalidTimeZoneID, errorprone:InvalidZoneId | `ZoneId.of("Europe/Berlinn")` |  |
| B167 | Dreibuchstabige Zeitzonen-ID | 2 | – | errorprone:ThreeLetterTimeZoneID | `TimeZone.getTimeZone("PST")` → `"America/Los_Angeles"` |  |
| B168 | Collection mit sich selbst als Argument ★ | 1 | – | errorprone:ModifyingCollectionWithItself, intellij:CollectionAddedToSelf | `list.addAll(list)` |  |
| B169 | Collection in for-each oder eigenem Stream verändert | 2* | – | errorprone:ModifyCollectionInEnhancedForLoop, errorprone:ModifySourceCollectionInStream | `for (A a : list) list.remove(a);` |  |
| B170 | list.remove(i) in aufsteigender Zählschleife | 2* | – | intellij:SuspiciousListRemoveInLoop | `for (int i = 0; i < list.size(); i++) list.remove(i);` |  |
| B171 | List.remove mit int bei List<Integer> | 2* | – | errorprone:ListRemoveAmbiguous | `List<Integer> l; l.remove(i)` |  |
| B172 | Unpassender Argumenttyp für Collection-/Arrays-Methode | 2* | – | intellij:SuspiciousMethodCalls, eclipse:COMPILER_PB_UNLIKELY_COLLECTION_METHOD_ARGUMENT_TYPE, pmd:CollectionTypeMismatch, intellij:SuspiciousArrayMethodCall | `List<String> l; l.contains(42)` |  |
| B173 | Temporäre Collection in-place geändert | 2 | – | errorprone:UnusedCollectionModifiedInPlace | `Collections.sort(new ArrayList<>(list))` |  |
| B174 | null statt leerer Collection zurückgegeben | 1 | ⚠️ | pmd:ReturnEmptyCollectionRatherThanNull, errorprone:ReturnsNullCollection | `List<X> f() { return null; }` → `return List.of();` |  |
| B175 | Hashtable.contains statt containsValue | 2* | ✅ | errorprone:HashtableContains | `table.contains(v)` → `table.containsValue(v)` |  |
| B176 | Properties mit put/get statt setProperty/getProperty | 2* | ⚠️ | intellij:UseOfPropertiesAsHashtable | `p.put("k", "v")` → `p.setProperty("k", "v")` |  |
| B177 | IdentityHashMap mit Wrapper-Schlüssel | 2 | – | errorprone:IdentityHashMapBoxing | `new IdentityHashMap<Integer, V>()` |  |
| B178 | IdentityHashMap mit normaler Map vermischt | 2* | – | errorprone:IdentityHashMapUsage | `IdentityHashMap<K, V> m; m.putAll(hashMap)` |  |
| B179 | Sortierte Collection mit nicht vergleichbarem Typ | 2* | – | errorprone:Incomparable | `new TreeSet<Foo>() bei Foo ohne Comparable` |  |
| B180 | Multimap.keys() statt keySet() | 2* | ✅ | errorprone:MultimapKeys | `for (K k : mm.keys())` → `mm.keySet()` | Guava |
| B181 | stream::iterator als Iterable | 2* | – | errorprone:StreamToIterable | `for (A a : (Iterable<A>) s::iterator)` |  |
| B182 | toString auf Stream | 2* | – | errorprone:StreamToString | `stream.toString()` |  |
| B183 | Unendlicher Stream ohne Begrenzung | 2 | – | intellij:EndlessStream | `Stream.iterate(0, i -> i + 1).collect(toList())` |  |
| B184 | null an Optional-Variable, -Rückgabe oder -Parameter | 2* | ⚠️ | intellij:OptionalAssignedToNull, errorprone:NullOptional | `Optional<String> o = null;` → `Optional.empty()` |  |
| B185 | null-Literal an null-feindliche Methode | 2* | – | errorprone:DangerousLiteralNull | `opt.orElseGet(null)` |  |
| B186 | get() auf als leer geprüftes Optional | 2* | – | errorprone:OptionalNotPresent | `if (!opt.isPresent()) { opt.get(); }` |  |
| B187 | Optional.map mit verworfenem Ergebnis | 2* | ✅ | errorprone:OptionalMapUnusedValue | `opt.map(x -> f(x));` → `opt.ifPresent(x -> f(x));` |  |
| B188 | Optional.map liefert verschachteltes Optional | 2* | – | errorprone:OptionalMapToOptional | `opt.map(x -> Optional.of(x))` → `flatMap` |  |
| B189 | Optional.ofNullable mit bekanntem Argument | 2 | ✅ | intellij:OptionalOfNullableMisuse | `Optional.ofNullable(null)` → `Optional.empty()` |  |
| B190 | orElseThrow-Supplier wirft statt zurückzugeben | 1 | ✅ | baseline:OptionalOrElseThrowThrows, intellij:ThrowableSupplierOnlyThrowException | `orElseThrow(() -> { throw new X(); })` → `orElseThrow(() -> new X())` |  |
| B191 | CacheLoader.load gibt null zurück | 2 | – | errorprone:CacheLoaderNull | `V load(K k) { return null; }` | Guava |
| B192 | Ordering.explicit ohne alle Enum-Konstanten | 2* | – | picnic:ExplicitEnumOrdering | `Ordering.explicit(Color.RED, Color.GREEN)` | Guava |
| B193 | ImmutableMap.Builder.build() statt buildOrThrow() | 2* | ✅ | baseline:ImmutableMapDuplicateKeyStrategy | `builder.build()` → `builder.buildOrThrow()` | Guava |
| B194 | Futures.getChecked mit ungeeignetem Exception-Typ | 2 | – | errorprone:FuturesGetCheckedIllegalExceptionType | `Futures.getChecked(f, RuntimeException.class)` | Guava |
| B195 | checkNotNull mehrfach auf dieselbe Variable | 2 | – | errorprone:CheckNotNullMultipleTimes | `checkNotNull(a); checkNotNull(a);` | Guava |
| B196 | RateLimiter mit Warmup 0 | 2 | – | baseline:ZeroWarmupRateLimiter | `RateLimiter.create(10, Duration.ZERO)` | Guava |
| B197 | Doppelte oder bereits geprüfte Bedingung | 1 | – | intellij:DuplicateCondition, errorprone:AlreadyChecked | `if (x) … else if (x) …; if (a) { if (a) … }` | Ergänzt B011 |
| B198 | Zweige mit identischem Inhalt | 1 | ⚠️ | errorprone:DuplicateBranches, pmd:IdenticalConditionalBranches, intellij:ConditionalExpressionWithIdenticalBranches, intellij:IfStatementWithIdenticalBranches | `if (c) f(); else f();` → `f();` |  |
| B199 | Schleife ohne Ausgang | 1 | – | intellij:InfiniteLoopStatement | `for (;;) { work(); }` |  |
| B200 | Unbedingte Endlosrekursion | 2 | – | errorprone:InfiniteRecursion, netbeans:Infinite recursion (bugs), intellij:InfiniteRecursion | `int f(int x) { return f(x); }` |  |
| B201 | Schleifenbedingung wird in der Schleife nie geändert | 2 | – | errorprone:LoopConditionChecker, intellij:LoopConditionNotUpdatedInsideLoop | `while (i < n) { work(); }` |  |
| B202 | Schleifenrumpf läuft höchstens einmal | 1 | – | intellij:LoopStatementsThatDontLoop, pmd:AvoidBranchingStatementAsLastInLoop | `for (X x : xs) { return x; }` |  |
| B203 | Schleifenindex läuft in falsche Richtung | 1 | – | intellij:OverflowingLoopIndex | `for (int i = 0; i < n; i--)` |  |
| B204 | float als Schleifenzähler | 1 | – | pmd:DontUseFloatTypeForLoopIndices | `for (float f = 0; f < 1; f += 0.1f)` |  |
| B205 | Label statt case im switch ★ | 1 | – | intellij:TextLabelInSwitchStatement, pmd:NonCaseLabelInSwitch, netbeans:Possibly missing switch "case" statement (bugs) | `switch (x) { caes1: … }` |  |
| B206 | Label an Anweisung, die keine Schleife ist | 1 | – | errorprone:LabelledBreakTarget | `label: { … break label; }` |  |
| B207 | Im case deklarierte Variable in anderem case benutzt | 1 | – | intellij:SwitchStatementWithConfusingDeclaration | `case 1: int x = 1; break; case 2: x = 2;` |  |
| B208 | switch über Enum ohne alle Konstanten | 2* | – | errorprone:MissingCasesInEnumSwitch, intellij:EnumSwitchStatementWhichMissesCases, eclipse:COMPILER_PB_INCOMPLETE_ENUM_SWITCH, eclipse:COMPILER_PB_MISSING_ENUM_CASE_DESPITE_DEFAULT | `switch (dir) { case N: … }` | Option: auch trotz default melden |
| B209 | default in vollständigem Enum-/sealed-switch | 2* | ⚠️ | pmd:ExhaustiveSwitchHasDefault, errorprone:UnnecessaryDefaultInEnumSwitch, intellij:UnnecessaryDefault | `switch (dir) { case N -> 1; case S -> 2; default -> 0; }` | Gegenregel zu B005 |
| B210 | Auf null geprüfte Variable im if nicht benutzt | 1 | – | intellij:VariableNotUsedInsideIf | `if (x != null) { return y; }` |  |
| B211 | null geprüft, dann equals auf anderem Objekt | 1 | ⚠️ | pmd:UnusedNullCheckInEquals | `if (x != null && y.equals(x))` → `x.equals(y)` |  |
| B212 | Dereferenzierung eines Ausdrucks mit null-Zweig | 1 | – | errorprone:DereferenceWithNullBranch | `(c ? null : s).length()` |  |
| B213 | Cast passt nicht zur vorigen instanceof-Prüfung | 1 | – | intellij:CastConflictsWithInstanceof, errorprone:InstanceOfAndCastMatchWrongType | `if (o instanceof A) { ((B) o).run(); }` |  |
| B214 | instanceof auf eigenen Typ ist Null-Prüfung | 2* | ✅ | errorprone:BadInstanceof | `String s; s instanceof String` → `s != null` |  |
| B215 | Class.isInstance mit Class-Argument | 2* | – | errorprone:IsInstanceOfClass | `Foo.class.isInstance(Bar.class)` |  |
| B216 | getClass() auf Class-Objekt | 2* | ✅ | intellij:ClassGetClass, errorprone:GetClassOnClass | `Foo.class.getClass()` → `Foo.class` |  |
| B217 | getClass() auf Enum-Konstante | 2* | ✅ | errorprone:GetClassOnEnum | `Color.RED.getClass()` → `getDeclaringClass()` |  |
| B218 | Ergebnis von new verworfen | 1 | – | intellij:ResultOfObjectAllocationIgnored, netbeans:Result of new Object ignored (bugs), eclipse:COMPILER_PB_UNUSED_OBJECT_ALLOCATION, errorprone:UnusedAnonymousClass | `new Foo();` |  |
| B219 | Exception erzeugt, aber nicht geworfen ★ | 2 | ⚠️ | errorprone:DeadException, intellij:ThrowableNotThrown, netbeans:Created Throwable not thrown (bugs) | `new IllegalStateException("x");` → `throw new …` |  |
| B220 | Ergebnis einer seiteneffektfreien Methode ignoriert | 2* | – | errorprone:ReturnValueIgnored, intellij:IgnoreResultOfCall, pmd:UnusedReturnValue, pmd:UselessPureMethodCall, pmd:UselessOperationOnImmutable, pmd:AvoidLosingExceptionInformation | `s.trim(); bd.add(x); e.getMessage();` |  |
| B221 | Rückgabewert von read/skip/ResultSet.next ignoriert | 2* | – | baseline:ReadReturnValueIgnored, pmd:CheckSkipResult, pmd:CheckResultSet | `in.read(buf); in.skip(10); rs.next();` |  |
| B222 | Setter mit Wert aus eigenem Getter | 1 | – | errorprone:SelfSet, intellij:PropertyValueSetToItself | `b.setX(b.getX())` |  |
| B223 | Setter in derselben Kette doppelt aufgerufen | 1 | – | errorprone:RedundantSetterCall | `b.setX(1).setX(2)` |  |
| B224 | Aufruf mit konstantem Argument wirft immer | 2 | – | errorprone:AlwaysThrows | `UUID.fromString("foo")` |  |
| B225 | Konstruktor ruft überschreibbare oder abstrakte Methode auf | 2 | – | pmd:ConstructorCallsOverridableMethod, netbeans:Problematic call in the constructor (initialization), intellij:OverridableMethodCallDuringObjectConstruction, intellij:AbstractMethodCallInConstructor | `A() { init(); } public void init() {…}` |  |
| B226 | this entweicht im Konstruktor | 2* | – | javac:this-escape, netbeans:Passing suspicious parameter in the constructor (initialization), intellij:ThisEscapedInConstructor | `A() { registry.add(this); }` |  |
| B227 | Statisches Feld in Konstruktor oder Instanzmethode zugewiesen | 2 | – | errorprone:StaticAssignmentInConstructor, pmd:AssignmentToNonFinalStatic, intellij:AssignmentToStaticFieldFromInstanceMethod | `A() { instance = this; }` |  |
| B228 | Statische Initialisierung liest nicht-finale statische Variable | 2 | – | intellij:NonFinalStaticVariableUsedInClassInitialization, netbeans:Static non-final variable used during initialization (initialization) | `static int a = b + 1; static int b = 2;` |  |
| B229 | Feld vor seiner Initialisierung gelesen | 2 | – | javac:compiler.warn.forward.ref, javac:compiler.warn.self.ref | `int a = this.b + 1; int b = 2;` |  |
| B230 | Initialisiertes transient-Feld ohne readObject | 1 | – | intellij:TransientFieldNotInitialized | `private transient List<X> l = new ArrayList<>();` |  |
| B231 | LazyConstant-Feld nicht final | 2 | – | intellij:IncorrectLazyConstantUsage | `private LazyConstant<X> c = LazyConstant.of(…);` → `private final …` | ab 25 (Preview) |
| B232 | Record-Accessor im kompakten Konstruktor | 1 | ✅ | errorprone:RecordAccessorInCompactConstructor | `record R(int x) { R { check(x()); } }` → `check(x)` |  |
| B233 | Record mit Array-Komponente | 1 | – | baseline:DangerousRecordArrayField, errorprone:ArrayRecordComponent | `record R(int[] data) {}` |  |
| B234 | value class ohne equals/hashCode | 1 | – | errorprone:ValueClassEquals | `value class P { int x; }` | JEP 401, noch Preview |
| B235 | var mit Diamond-Operator inferiert Object | 1 | – | netbeans:var used with diamond operator (bugs) | `var l = new ArrayList<>();` |  |
| B236 | Roher Typ ohne Typargumente | 2* | – | javac:rawtypes, eclipse:COMPILER_PB_RAW_TYPE_REFERENCE, intellij:RawUseOfParameterizedType, baseline:RawTypes | `List list = new ArrayList();` → `List<String>` |  |
| B237 | Varargs-Parameter in @SafeVarargs-Methode neu zugewiesen | 1 | – | intellij:SafeVarargsHasNoEffect | `@SafeVarargs static <T> void f(T... xs) { xs = …; }` |  |
| B238 | catch-Parameter neu zugewiesen | 1 | – | intellij:AssignmentToCatchBlockParameter, netbeans:Assignment to catch-block parameter (assignment_issues), pmd:AvoidReassigningCatchVariables | `catch (IOException e) { e = new IOException(); }` | Ergänzt B013 |
| B239 | Lokale Variable für unabhängigen Zweck wiederverwendet | 2 | – | intellij:ReuseOfLocalVariable | `String s = name(); use(s); s = path(); use(s);` |  |
| B240 | Serializable-Klasse ohne serialVersionUID | 2* | – | eclipse:COMPILER_PB_MISSING_SERIAL_VERSION, javac:compiler.warn.missing.SVUID, pmd:MissingSerialVersionUID, intellij:SerializableHasSerialVersionUIDField, intellij:SerializableInnerClassHasSerialVersionUIDField | `class A implements Serializable {}` |  |
| B241 | serialVersionUID nicht private static final long ★ | 1 | ✅ | intellij:SerialVersionUIDNotStaticFinal, javac:compiler.warn.improper.SVUID, javac:compiler.warn.long.SVUID, javac:compiler.warn.constant.SVUID, javac:serial | `long serialVersionUID = 1L;` → `private static final long serialVersionUID = 1L;` |  |
| B242 | serialPersistentFields nicht private static final | 1 | ✅ | javac:compiler.warn.improper.SPF, intellij:SerialPersistentFieldsWithWrongSignature | `ObjectStreamField[] serialPersistentFields = {};` → `private static final …` |  |
| B243 | Serialisierungsmethode mit falscher Signatur (nicht private, static) | 1 | ✅ | javac:compiler.warn.serial.method.not.private, intellij:ReadObjectAndWriteObjectPrivate, javac:compiler.warn.serial.method.static | `void writeObject(ObjectOutputStream out)` → `private void writeObject(…)` |  |
| B244 | Serialisierungs-Member in nicht serialisierbarer Klasse | 2* | – | intellij:NonSerializableWithSerialVersionUIDField, intellij:NonSerializableWithSerializationMethods | `class A { private static final long serialVersionUID = 1L; }` |  |
| B245 | Serialisierungs-Member in Enum oder Record wirkungslos | 1 | – | javac:compiler.warn.ineffectual.serial.field.enum, javac:compiler.warn.ineffectual.serial.method.enum, javac:compiler.warn.ineffectual.serial.method.record, intellij:SerializableRecordContainsIgnoredMembers | `enum E { A; private void writeObject(ObjectOutputStream o) {} }` |  |
| B246 | Externalizable mit readObject/writeObject | 2 | – | intellij:ExternalizableWithSerializationMethods | `class C implements Externalizable { private void readObject(…) }` |  |
| B247 | Externalizable ohne öffentlichen No-Arg-Konstruktor | 2* | – | intellij:ExternalizableWithoutPublicNoArgConstructor | `class A implements Externalizable { A(int x) {} }` |  |
| B248 | Serialisierbare innere Klasse in nicht serialisierbarer äußerer | 2* | – | intellij:SerializableInnerClassWithNonSerializableOuterClass | `class Outer { class Inner implements Serializable {} }` |  |
| B249 | @Serial an ungeeignetem Member | 2* | – | intellij:SerialAnnotationUsedOnWrongMember | `@Serial private void foo()` |  |
| B250 | clone() in Klasse ohne Cloneable | 2* | – | pmd:CloneMethodMustImplementCloneable, intellij:CloneInNonCloneableClass, netbeans:clone() in a non-Cloneable class (bugs) | `class A { public A clone() {…} }` | Ergänzt B034 |
| B251 | Cloneable ohne überschriebenes clone() | 1 | – | intellij:CloneableImplementsClone, netbeans:Cloneable class does not implement clone(). (bugs) | `class A implements Cloneable { }` |  |
| B252 | Expliziter Aufruf von finalize() | 1 | – | pmd:AvoidCallingFinalize, netbeans:finalize() called explicitly (finalization), intellij:NoExplicitFinalizeCalls | `obj.finalize();` |  |
| B253 | Interface-Methode kollidiert mit Object.clone/finalize | 1 | – | intellij:InterfaceMethodClashesWithObject | `interface A { int clone(); }` |  |
| B254 | Feld einer inneren/anonymen Klasse verdeckt äußere Variable | 2 | – | intellij:InnerClassVariableHidesOuterClassVariable, intellij:AnonymousClassVariableHidesContainingMethodVariable | `class O { int x; class I { int x; } }` |  |
| B255 | Feld verdeckt Feld der Oberklasse | 2* | – | intellij:FieldHidesSuperclassField, errorprone:HidingField | `class B extends A { int x; } // A hat x` |  |
| B256 | Typparameter verdeckt sichtbaren Typ | 2 | – | intellij:TypeParameterHidesVisibleType, eclipse:COMPILER_PB_TYPE_PARAMETER_HIDING, errorprone:TypeNameShadowing, errorprone:TypeParameterShadowing | `class MyList<String> {}` |  |
| B257 | Typparameter als Qualifier | 1 | – | errorprone:TypeParameterQualifier | `T.Inner` |  |
| B258 | Unbenutzter Typparameter | 1 | ⚠️ | eclipse:COMPILER_PB_UNUSED_TYPE_PARAMETER, errorprone:UnusedTypeParameter | `<T> void f(String s)` → `void f(String s)` |  |
| B259 | Unbenutzte private verschachtelte Klasse | 2 | ⚠️ | errorprone:UnusedNestedClass | `private static class Helper {}` | Ergänzt B027 |
| B260 | Klasse implementiert Annotations-Interface | 2 | – | intellij:ExtendsAnnotation, netbeans:Don't use Annotation as super interface (rules15), eclipse:COMPILER_PB_ANNOTATION_SUPER_INTERFACE | `class Impl implements Override` |  |
| B261 | Annotation-Implementierung ohne equals/hashCode | 2 | – | errorprone:BadAnnotationImplementation | `class A implements Annotation { … }` |  |
| B262 | Reflektive Abfrage einer Annotation ohne RUNTIME-Retention | 2* | – | errorprone:NonRuntimeAnnotation, intellij:ReflectionForUnavailableAnnotation | `c.getAnnotation(SourceAnno.class)` |  |
| B263 | Annotation-Processor gibt true aus process zurück | 2 | – | errorprone:DoNotClaimAnnotations | `boolean process(…) { return true; }` |  |
| B264 | @SuppressWarnings("deprecated") statt "deprecation" | 1 | ✅ | errorprone:SuppressWarningsDeprecated | `@SuppressWarnings("deprecated")` → `"deprecation"` |  |
| B265 | Method.invoke-Argumente passen nicht zur Signatur | 2* | – | intellij:JavaReflectionInvocation | `Method m = c.getMethod("f", String.class); m.invoke(o, 1, 2)` |  |
| B266 | Cast des Ergebnisses von newInstance statt asSubclass | 2 | ⚠️ | errorprone:UnsafeReflectiveConstructionCast | `(Foo) cls.getDeclaredConstructor().newInstance()` → `asSubclass(Foo.class)` |  |
| B267 | Cleaner-Aktion hält das registrierte Objekt fest | 2* | – | intellij:CapturingCleaner | `cleaner.register(this, () -> this.close())` |  |
| B268 | assert mit konstanter Bedingung | 1 | ⚠️ | intellij:ConstantAssertCondition, errorprone:AssertFalse | `assert false;` → `throw new AssertionError();` |  |
| B269 | assert mit Seiteneffekt | 1 | – | intellij:AssertWithSideEffects, netbeans:Assert with side effects (bugs) | `assert list.add(x);` |  |
| B270 | assert-Meldung ist kein String | 2* | – | intellij:AssertMessageNotString | `assert ok : 42;` |  |
| B271 | Aufruf von System.exit()/Runtime.halt() | 1 | – | intellij:SystemExit, pmd:DoNotTerminateVM, errorprone:SystemExitOutsideMain | `System.exit(1);` | Option: in main erlauben |
| B272 | Thread.dumpStack() verwendet | 2 | – | intellij:ThreadDumpStack, netbeans:Thread Dump Stack (code_maturity) | `Thread.dumpStack();` | Ergänzt B022 |
| B273 | Null-Prüfung von System.console() | 2 | – | errorprone:SystemConsoleNull | `if (System.console() == null)` | ab 22 |
| B274 | main-Methode mit falscher Signatur | 1 | ⚠️ | errorprone:IncorrectMainMethod | `static int main(String[] a)` |  |
| B275 | Character.getNumericValue mit überraschender Semantik | 2 | – | errorprone:CharacterGetNumericValue | `Character.getNumericValue(c)` → `Character.digit(c, 10)` |  |
| B276 | InputStream.read() liefert byte statt 0..255 | 2* | ✅ | intellij:SuspiciousReturnByteInputStream | `return b;` → `return b & 0xFF;` |  |
| B277 | JDBC-Spaltenindex 0 | 2* | – | intellij:ResultSetIndexZero, netbeans:Incorrect column index in ResultSet (bugs) | `rs.getString(0)` |  |
| B278 | FileSystems.newFileSystem(path, null) mehrdeutig | 2 | ✅ | errorprone:NewFileSystem | `newFileSystem(p, null)` → `newFileSystem(p, (ClassLoader) null)` |  |
| B279 | Hostname auf nur eine IP-Adresse aufgelöst | 2 | – | errorprone:AddressSelection | `InetAddress.getByName(host)` → `getAllByName(host)` |  |
| B280 | throw null ★ | 1 | – | errorprone:ThrowNull, intellij:NullThrown | `throw null;` → `throw new IllegalStateException()` |  |
| B281 | throwIfUnchecked mit bekanntem Exception-Typ | 2 | ✅ | errorprone:ThrowIfUncheckedKnownChecked, errorprone:ThrowIfUncheckedKnownUnchecked | `catch (RuntimeException e) { throwIfUnchecked(e); }` → `throw e;` | Guava |
| B282 | AutoCloseable/Ressourcen-Stream nicht in try-with-resources | 2* | ⚠️ | intellij:AutoCloseableResource, errorprone:StreamResourceLeak | `Files.lines(p).count()` → `try (Stream<String> s = Files.lines(p))` |  |
| B283 | try-with-resources schließt System.out/err | 2 | – | errorprone:ClosingStandardOutputStreams | `try (PrintStream p = System.out) {}` |  |
| B284 | Bitmaske mit > 0 statt != 0 geprüft | 1 | ⚠️ | eclipse-cleanup:CHECK_SIGN_OF_BITWISE_OPERATION | `(flags & MASK) > 0` → `(flags & MASK) != 0` | falsch, wenn die Maske das Vorzeichenbit enthält |

### E: Exceptions und Ressourcen (neu)

| Code | Regel | Stufe | Fix | Herkunft | Beispiel | Hinweis |
|---|---|---|---|---|---|---|
| E001 | Klasse erweitert Error oder Throwable direkt | 1 | – | baseline:ExtendsErrorOrThrowable, intellij:ExtendsThrowable, pmd:DoNotExtendJavaLangError, pmd:DoNotExtendJavaLangThrowable | `class MyError extends Error {}` |  |
| E002 | Eigene checked Exception-Klasse | 2 | – | intellij:CheckedExceptionClass | `class MyException extends Exception` | Gegenrichtung möglich; umstritten |
| E003 | Error statt RuntimeException geworfen | 1 | – | baseline:ThrowError | `throw new Error("x")` → `throw new IllegalStateException("x")` |  |
| E004 | NullPointerException explizit geworfen | 1 | – | pmd:AvoidThrowingNullPointerException | `throw new NullPointerException();` → `Objects.requireNonNull(x)` |  |
| E005 | Exception ohne Meldung erzeugt | 2 | – | intellij:NewExceptionWithoutArguments | `throw new IllegalStateException();` |  |
| E006 | Throwable in statischem Feld | 2 | – | errorprone:StaticAssignmentOfThrowable | `static final Exception E = new Exception();` |  |
| E007 | catch-Block wirft Exception nur weiter | 1 | ⚠️ | pmd:AvoidRethrowingException, intellij:CaughtExceptionImmediatelyRethrown | `catch (IOException e) { throw e; }` |  |
| E008 | Exception in neue Instanz desselben Typs gewickelt | 1 | ⚠️ | pmd:AvoidThrowingNewInstanceOfSameException | `catch (IOException e) { throw new IOException(e); }` |  |
| E009 | IOException in RuntimeException statt UncheckedIOException | 1 | ⚠️ | baseline:PreferUncheckedIoException | `throw new RuntimeException(e)` → `new UncheckedIOException(e)` |  |
| E010 | ReflectiveOperationException als AssertionError weitergeworfen | 2 | ✅ | errorprone:RethrowReflectiveOperationExceptionAsLinkageError | `throw new AssertionError(e)` → `throw new LinkageError(e.getMessage(), e)` |  |
| E011 | initCause statt Konstruktor mit Ursache | 2* | ✅ | intellij:UnnecessaryInitCause, netbeans:Unnecessary Throwable.initCause (general) | `new IllegalStateException("m").initCause(e)` → `new IllegalStateException("m", e)` |  |
| E012 | instanceof auf gefangene Exception statt eigenem catch | 1 | ⚠️ | intellij:InstanceofCatchParameter, pmd:AvoidInstanceofChecksInCatchClause | `catch (Exception e) { if (e instanceof IOException) … }` |  |
| E013 | Exception im selben try geworfen und gefangen | 2* | – | pmd:ExceptionAsFlowControl, intellij:ThrowCaughtLocally | `try { throw new X(); } catch (X e) { … }` |  |
| E014 | Error gefangen und nicht weitergeworfen | 2 | – | intellij:ErrorRethrown | `catch (OutOfMemoryError e) { log(e); }` |  |
| E015 | Ressource im try-Rumpf nie verwendet | 1 | – | javac:compiler.warn.try.resource.not.referenced | `try (var lock = acquire()) { work(); }` |  |

### S: Vereinfachungen

| Code | Regel | Stufe | Fix | Herkunft | Beispiel | Hinweis |
|---|---|---|---|---|---|---|
| S023 | Annotation-Syntax vereinfachbar (value=, Einzel-Array, leere Klammern) ★ | 1 | ✅ | pmd:UnnecessaryAnnotationValueElement, intellij:SimplifiableAnnotation, picnic:CanonicalAnnotationSyntax | `@SuppressWarnings(value = {"x"})` → `@SuppressWarnings("x")` |  |
| S024 | Annotationsparameter mit Default-Wert | 2* | ✅ | intellij:DefaultAnnotationParam | `@Test(timeout = 0L)` → `@Test` | nur Annotationen der Datei oder bekannte JDK/JUnit-Annotationen |
| S025 | @Deprecated an lokaler Variable/Parameter wirkungslos | 1 | ✅ | javac:compiler.warn.deprecated.annotation.has.no.effect, errorprone:DeprecatedVariable | `@Deprecated int x = 1;` → `int x = 1;` |  |
| S026 | @Override an Record-Komponente wirkungslos | 1 | ✅ | errorprone:RecordComponentOverride | `record R(@Override int x)` → `record R(int x)` |  |
| S027 | @SafeVarargs bei reifizierbarem Varargs-Typ | 2 | ✅ | intellij:SafeVarargsOnNonReifiableType | `@SafeVarargs static void f(String... s)` → `static void f(String... s)` |  |
| S028 | Überflüssiges extends Object (Klasse, Typparameter) ★ | 1 | ✅ | pmd:ExtendsObject, intellij:ExtendsObject, errorprone:ExtendsObject, intellij:TypeParameterExtendsObject | `class A extends Object` → `class A; <T extends Object>` → `<T>` |  |
| S029 | Interface schon durch Oberklasse implementiert | 2* | ✅ | eclipse:COMPILER_PB_REDUNDANT_SUPERINTERFACE, pmd:UnnecessaryInterfaceDeclaration | `class B extends A implements I (A implements I)` → `class B extends A` | nur wenn die Oberklasse in der Datei steht |
| S030 | Überflüssiger Default-Konstruktor | 1 | ✅ | intellij:UnnecessaryConstructor, pmd:UnnecessaryConstructor | `public class A { public A() {} }` → `public class A {}` | nur einziger Konstruktor, gleiche Sichtbarkeit wie die Klasse, ohne Javadoc |
| S031 | Überflüssiger Record-Konstruktor | 1 | ✅ | intellij:RedundantRecordConstructor | `record P(int x) { public P {} }` → `record P(int x) {}` |  |
| S032 | Leerer Methoden-/Konstruktorrumpf ohne Kommentar | 1 | – | pmd:UncommentedEmptyConstructor, pmd:UncommentedEmptyMethodBody | `void f() {}` |  |
| S033 | Überschreibende Methode ruft nur super auf | 1 | ⚠️ | intellij:RedundantMethodOverride, errorprone:RedundantOverride, pmd:UselessOverridingMethod | `@Override void f(int x) { super.f(x); }` → `entfernen` | Fix unsafe: Annotationen/Javadoc/Sichtbarkeit können abweichen |
| S034 | static-Feld unnötig transient | 1 | ✅ | errorprone:TransientMisuse | `static transient int x;` → `static int x;` |  |
| S035 | transient-Feld in nicht serialisierbarer Klasse | 2* | ✅ | intellij:TransientFieldInNonSerializableClass | `class A { private transient int x; }` → `private int x;` | nur Klassen ohne Oberklasse/Interfaces außerhalb der Datei |
| S036 | Doppelte Exception in throws-Liste ★ | 1 | ✅ | intellij:DuplicateThrows | `throws IOException, IOException` → `throws IOException` |  |
| S037 | Unchecked Exception in throws-Klausel | 2* | ✅ | errorprone:ThrowsUncheckedException, pmd:AvoidUncheckedExceptionsInSignatures | `void f() throws IllegalArgumentException` → `void f()` | nur bekannte JDK-Exceptions; @throws im Javadoc bleibt |
| S038 | Typargument bei nicht-generischer Methode | 2* | ✅ | errorprone:UnnecessaryTypeArgument | `this.<String>f() bei void f()` → `this.f()` | nur Methoden der Datei |
| S039 | Lambda/anonyme Klasse als Konstante statt Methode | 2 | ⚠️ | errorprone:UnnecessaryAnonymousClass, errorprone:UnnecessaryLambda | `static final Predicate<A> P = a -> …;` → `static boolean p(A a)` |  |
| S040 | Triviale Konstante (EMPTY = "") | 1 | ✅ | errorprone:InlineTrivialConstant | `private static final String EMPTY = "";` → `""` |  |
| S041 | Literal wiederholt vorhandene Konstante | 2 | ✅ | errorprone:DeduplicateConstants | `static final String K = "k"; … f("k")` → `f(K)` |  |
| S042 | Feldzuweisung aus Initializer/Konstruktor in die Deklaration | 2 | ⚠️ | intellij:MoveFieldAssignmentToInitializer | `int x; { x = 1; }` → `int x = 1;` |  |
| S043 | Deklaration und erste Zuweisung zusammenfassen | 1 | ✅ | intellij:JoinDeclarationAndAssignmentJava, errorprone:InitializeInline | `int x; x = 1;` → `int x = 1;` |  |
| S044 | Rückgabewert getrennt vom return berechnet | 2 | ⚠️ | intellij:ReturnSeparatedFromComputation | `int r = a + b; log(); return r;` → `log(); return a + b;` | Erweiterung von S005 (dazwischenliegende Anweisungen) |
| S045 | Kommentar als Blockende-Markierung | 1 | ✅ | intellij:BlockMarkerComments | `} // end while` → `}` |  |
| S046 | Nicht standardkonformer Parameterkommentar | 1 | ✅ | errorprone:ParameterComment | `f(true /* enabled */)` → `f(/* enabled= */ true)` |  |
| S047 | Statisches Member über Instanz statt Typname ★ | 2* | ✅ | eclipse:COMPILER_PB_STATIC_ACCESS_RECEIVER, errorprone:StaticQualifiedUsingExpression, intellij:AccessStaticViaInstance, javac:compiler.warn.static.not.qualified.by.type, javac:static, netbeans:Accessing static via reference (general) | `thread.sleep(10)` → `Thread.sleep(10)` |  |
| S048 | Eigene Klasse als Qualifier überflüssig (statisches Member, innere Klasse) | 2 | ✅ | intellij:UnnecessarilyQualifiedInnerClassAccess, intellij:UnnecessarilyQualifiedStaticUsage | `class Foo { void f() { Foo.bar(); } }` → `bar()` |  |
| S049 | Statisch importiertes Member trotzdem qualifiziert | 2 | ✅ | intellij:UnnecessarilyQualifiedStaticallyImportedElement | `import static java.lang.Math.max; Math.max(a, b)` → `max(a, b)` | nahe I005 |
| S050 | Unnötig qualifiziertes this | 1 | ✅ | pmd:UselessQualifiedThis, intellij:UnnecessaryQualifierForThis | `Foo.this.x (im Rumpf von Foo)` → `this.x` |  |
| S051 | Unnötiges super. bei nicht überschriebenem Member | 2 | ⚠️ | intellij:UnnecessarySuperQualifier | `super.helper()` → `helper()` |  |
| S052 | Überflüssiges else nach return/throw/break/continue | 1 | ✅ | eclipse:COMPILER_PB_UNNECESSARY_ELSE, intellij:ConfusingElse | `if (a) return 1; else return 2;` → `if (a) return 1; return 2;` |  |
| S053 | else-Block enthält nur ein if | 1 | ✅ | netbeans:DN_JoinElseIf (suggestions) | `else { if (c) {…} }` → `else if (c) {…}` |  |
| S054 | Negierte Bedingung mit else-Zweig (if/else, ?:) | 1 | ✅ | pmd:ConfusingTernary, intellij:NegatedConditional, intellij:NegatedIfElse | `if (!c) a(); else b();` → `if (c) b(); else a();` |  |
| S055 | Negierter Bedingungsausdruck | 1 | ✅ | intellij:NegatedConditionalExpression | `!(c ? a : b)` → `c ? !a : !b` |  |
| S056 | Überflüssiges break im switch ★ | 1 | ✅ | errorprone:UnnecessaryBreakInSwitch, intellij:UnnecessaryBreak | `case 1 -> { f(); break; }` → `case 1 -> f();` |  |
| S057 | Überflüssiges continue am Schleifenende ★ | 1 | ✅ | intellij:UnnecessaryContinue, netbeans:DN_RemoveUnnecessaryContinue (general), errorprone:RedundantControlFlow | `for (…) { f(); continue; }` → `for (…) { f(); }` |  |
| S058 | Überflüssiges Label an break/continue | 1 | ✅ | intellij:UnnecessaryLabelOnBreakStatement, netbeans:DN_RemoveUnnecessaryBreakLabel (general), intellij:UnnecessaryLabelOnContinueStatement, netbeans:DN_RemoveUnnecessaryContinueLabel (general) | `L: for (…) { break L; }` → `break;` |  |
| S059 | Unbenutztes Label ★ | 1 | ✅ | intellij:UnusedLabel, eclipse:COMPILER_PB_UNUSED_LABEL, errorprone:UnusedLabel, pmd:UnusedLabel | `L: for (…) { break; }` → `for (…) { break; }` |  |
| S060 | for ohne Initialisierung und Update statt while | 1 | ✅ | pmd:ForLoopShouldBeWhileLoop, intellij:ForLoopReplaceableByWhile | `for (; c; ) { … }` → `while (c) { … }` |  |
| S061 | Endlosschleife mit bedingtem break als Bedingung | 1 | ✅ | intellij:ConditionalBreakInInfiniteLoop, intellij:LoopWithImplicitTerminationCondition | `while (true) { if (done) break; … }` → `while (!done) { … }` |  |
| S062 | while mit dupliziertem Vorlauf statt do-while | 1 | ✅ | intellij:WhileCanBeDoWhile | `a(); while (c) { a(); }` → `do { a(); } while (c);` |  |
| S063 | do-while/while mit Literal-Bedingung | 1 | ✅ | pmd:WhileLoopWithLiteralBoolean | `do { … } while (false);` → `{ … }` |  |
| S064 | Überflüssige Längenprüfung vor Schleife über Array | 1 | ✅ | intellij:RedundantLengthCheck | `if (arr.length > 0) { for (var x : arr) … }` → `for (var x : arr) …` |  |
| S065 | Explizites close() am Ende von try-with-resources | 1 | ✅ | intellij:RedundantExplicitClose, javac:compiler.warn.try.explicit.close.call, javac:try | `try (var r = open()) { use(r); r.close(); }` → `ohne r.close()` |  |
| S066 | Switch-Regel-Block nur mit yield/Ausdruck | 1 | ✅ | errorprone:RefactorSwitch, intellij:RedundantLabeledSwitchRuleCodeBlock | `case A -> { yield "x"; }` → `case A -> "x";` |  |
| S067 | Doppelte Zweige im switch | 1 | ✅ | intellij:DuplicateBranchesInSwitch | `case 1 -> f(); case 2 -> f();` → `case 1, 2 -> f();` |  |
| S068 | switch mit zu wenigen Zweigen statt if | 1 | ⚠️ | intellij:SwitchStatementWithTooFewBranches, pmd:TooFewBranchesForSwitch | `switch (x) { case 1 -> a(); default -> b(); }` → `if (x == 1) a(); else b();` | unsafe: null-Verhalten bei String/Wrapper |
| S069 | Gemeinsamer Teilausdruck aller switch-Zweige | 1 | ✅ | intellij:SwitchExpressionCanBePushedDown | `case 0 -> f("a"); case 1 -> f("b");` → `f(switch (v) { case 0 -> "a"; case 1 -> "b"; })` |  |
| S070 | Negierter Vergleich / doppelte Negation ★ | 1 | ✅ | intellij:DoubleNegation, pmd:LogicInversion, intellij:NegatedEqualityExpression, picnic:EqualityRules.NotEqualTo+EqualToWithBoolean+BooleanIdentity, picnic:PrimitiveRules.LessThan+LessThanOrEqualTo+GreaterThan+GreaterThanOrEqualTo | `!(a == b)` → `a != b; !!b` → `b; !(a >= b)` → `a < b` | relationale Operatoren nur safe ohne Gleitkomma (NaN) |
| S071 | Vereinfachbarer boolescher Ausdruck | 1 | ✅ | intellij:SimplifiableBooleanExpression, picnic:AssortedRules.Or | `!(a ^ b)` → `a == b; a \|\| (!a && b)` → `a \|\| b` |  |
| S072 | Boolescher Ausdruck als Bedingungsausdruck kürzer | 1 | ✅ | intellij:BooleanExpressionMayBeConditional | `a && b \|\| !a && c` → `a ? b : c` |  |
| S073 | Boolescher Ausdruck lässt sich ausklammern | 1 | ✅ | intellij:ExpressionMayBeFactorized | `a && b \|\| a && c` → `a && (b \|\| c)` |  |
| S074 | Bedingung durch folgende Bedingung abgedeckt | 1 | ✅ | intellij:ConditionCoveredByFurtherCondition | `x != -1 && x > 0` → `x > 0` |  |
| S075 | Bereichsprüfung zu einer Prüfung vereinfachbar | 1 | ✅ | intellij:ExcessiveRangeCheck | `x >= 0 && x <= 0` → `x == 0` |  |
| S076 | Nicht-triviale konstante boolesche Ausdrücke | 1 | ✅ | errorprone:ComplexBooleanConstant | `if (1 > 2 && x)` → `if (false)` |  |
| S077 | Bedingungsausdruck mit konstanter Bedingung | 1 | ✅ | intellij:ConstantConditionalExpression | `true ? a : b` → `a` |  |
| S078 | Bedingungsausdruck in den Aufruf verschiebbar | 1 | ✅ | intellij:ConditionalCanBePushedInsideExpression | `c ? f(a) : f(b)` → `f(c ? a : b)` |  |
| S079 | Konstante/null auf der falschen Seite eines Vergleichs | 2 | ✅ | intellij:ConstantOnWrongSideOfComparison, picnic:NullRules.EqualToNull+NotEqualToNull+ObjectsIsNull+ObjectsNonNull | `null == x` → `x == null; Objects.isNull(x)` → `x == null` | Seite wählbar (Option, Gegenrichtung Yoda-Stil); Objects.isNull nur außerhalb von Methodenreferenzen |
| S080 | Variable mit durch == bekanntem Wert verwendet | 2 | ✅ | intellij:ConstantValueVariableUse | `if (x == 5) foo(x);` → `foo(5)` |  |
| S081 | Boolean.TRUE/FALSE statt Literal | 2* | ✅ | errorprone:BooleanLiteral | `boolean b = Boolean.TRUE;` → `boolean b = true;` |  |
| S082 | Boolesche Variable wird nur negiert gelesen | 2 | ⚠️ | intellij:BooleanVariableAlwaysNegated | `boolean b = …; if (!b) … while (!b) …` |  |
| S083 | Null-Prüfung auf sicher nicht-null Ausdruck | 2 | ✅ | errorprone:UnnecessaryCheckNotNull, errorprone:RedundantNullCheck, intellij:ObviousNullCheck | `Objects.requireNonNull(new Foo())` → `new Foo(); "x" == null` → `false` |  |
| S084 | null-Prüfung vor equals mit Konstante überflüssig | 1 | ✅ | intellij:SimplifiableEqualsExpression | `s != null && s.equals("a")` → `"a".equals(s)` |  |
| S085 | Null-sicheres equals manuell statt Objects.equals | 1 | ✅ | intellij:EqualsReplaceableByObjectsCall | `a != null && a.equals(b)` → `Objects.equals(a, b)` |  |
| S086 | Objects.equals mit sicher nicht-null erstem Argument | 2* | ✅ | intellij:ObjectsEqualsCanBeSimplified | `Objects.equals("a", s)` → `"a".equals(s)` |  |
| S087 | Enum mit equals statt == verglichen | 2* | ✅ | intellij:EqualsCalledOnEnumConstant, pmd:EnumComparison, picnic:EqualityRules.EqualToWithEnum | `Color.RED.equals(c)` → `Color.RED == c` | Fix safe nur mit Konstante als Empfänger (sonst NPE-Verhalten), nur Enums der Datei/JDK |
| S088 | compareTo() == 0 statt equals() | 1 | ⚠️ | intellij:CompareToEquality | `a.compareTo(b) == 0` → `a.equals(b)` |  |
| S089 | Überflüssiger compare-Aufruf | 2 | ✅ | intellij:RedundantCompareCall | `Integer.compare(a, b) == 0` → `a == b` |  |
| S090 | Handgeschriebener Vergleich statt Integer.compare | 2* | ✅ | intellij:UseCompareMethod | `a < b ? -1 : a > b ? 1 : 0` → `Integer.compare(a, b)` |  |
| S091 | Manuelles Minimum/Maximum statt Math.min/max | 1 | ⚠️ | intellij:ManualMinMaxCalculation | `a < b ? a : b` → `Math.min(a, b)` | unsafe: Gleitkomma (NaN, -0.0) |
| S092 | signum() mit 1/-1 statt mit 0 verglichen | 2 | ✅ | picnic:PrimitiveRules.IntegerSignumIsPositive+IntegerSignumIsNegative+LongSignumIsPositive+LongSignumIsNegative | `Integer.signum(i) == 1` → `Integer.signum(i) > 0` |  |
| S093 | compareTo(BigDecimal.ZERO) statt signum() | 2 | ✅ | picnic:BigDecimalRules.BigDecimalSignumEqualToZero+BigDecimalSignumIsPositive+BigDecimalSignumIsNegative | `x.compareTo(BigDecimal.ZERO) > 0` → `x.signum() > 0` |  |
| S094 | Sinnlose Arithmetik (+0, *1, /1, *0) | 2* | ✅ | intellij:PointlessArithmeticExpression | `int y = x * 1;` → `int y = x;` |  |
| S095 | Sinnlose Bitoperation (\| 0, ^ 0, << 0, & 0) | 1 | ✅ | intellij:PointlessBitwiseExpression, netbeans:Pointless bitwise expression (bitwise_operations) | `int y = x \| 0;` → `int y = x;` |  |
| S096 | Überflüssiges unäres Minus | 1 | ✅ | intellij:UnnecessaryUnaryMinus | `a + -b` → `a - b` |  |
| S097 | Zuweisung als Verbundzuweisung schreibbar | 1 | ✅ | intellij:ReplaceAssignmentWithOperatorAssignment, netbeans:Assignment replaceable with operator-assignment (assignment_issues) | `x = x + y;` → `x += y;` |  |
| S098 | Cast eines Literals statt passendem Literal | 1 | ✅ | intellij:DoubleLiteralMayBeFloatLiteral, errorprone:IntLiteralCast, intellij:IntLiteralMayBeLongLiteral | `(long) 42` → `42L; (float) 1.1` → `1.1f` |  |
| S099 | Gleitkommaliteral ohne Ziffern vor/nach dem Punkt | 1 | ✅ | intellij:ConfusingFloatingPointLiteral | `.5` → `0.5; 1.` → `1.0` |  |
| S100 | Lange Zahlenliterale ohne Unterstriche | 1 | ✅ | netbeans:Add Underscores (rules15), intellij:InsertLiteralUnderscores, pmd:UseUnderscoresInNumericLiterals | `1000000` → `1_000_000` | Mindestlänge konfigurierbar |
| S101 | Überflüssiger Cast | 2* | ✅ | intellij:RedundantCast, intellij:UnnecessaryExplicitNumericCast, eclipse:COMPILER_PB_UNNECESSARY_TYPE_CHECK, javac:cast, pmd:UnnecessaryCast | `String s = (String) "a";` → `String s = "a";` | nur Zuweisung/Initialisierung, nicht als Argument (Overloads) |
| S102 | Cast durch vorhandene Pattern-Variable ersetzbar ★ | 1 | ✅ | intellij:CastCanBeReplacedWithVariable | `if (o instanceof String s) ((String) o).length();` → `s.length()` | nur wenn o nicht neu zugewiesen |
| S103 | Explizites Boxing/Unboxing | 2* | ✅ | intellij:UnnecessaryBoxing, errorprone:UnnecessaryBoxedAssignment, intellij:UnnecessaryUnboxing, pmd:UnnecessaryBoxing, netbeans:Unnecessary boxing (rules15), netbeans:Unnecessary unboxing (rules15) | `Integer i = Integer.valueOf(5);` → `Integer i = 5; int n = i.intValue();` → `int n = i;` |  |
| S104 | Math-Aufruf mit konstantem Ergebnis | 2 | ✅ | intellij:ConstantMathCall | `Math.sin(0.0)` → `0.0` |  |
| S105 | Manuelle Long/Double-Hash-Berechnung | 1 | ✅ | intellij:UseHashCodeMethodInspection | `(int) (x ^ (x >>> 32))` → `Long.hashCode(x)` |  |
| S106 | hashCode() in Objects.hash | 2 | ✅ | errorprone:HashCodeInObjectsHash | `Objects.hash(x.hashCode())` → `Objects.hashCode(x)` |  |
| S107 | Verkettung mit leerem String | 1 | ✅ | intellij:TrivialStringConcatenation, pmd:AddEmptyString | `"" + n` → `String.valueOf(n)` |  |
| S108 | Überflüssiges toString()/String.valueOf() in Verkettung | 2* | ✅ | picnic:RedundantStringConversion, pmd:UselessStringValueOf, intellij:UnnecessaryCallToStringValueOf, intellij:UnnecessaryToStringCall | `"n=" + String.valueOf(n)` → `"n=" + n` | nicht bei char[]; toString() entfernen ist unsafe (NPE) |
| S109 | Überflüssige String-Operation (toString(), substring(0)) | 2* | ✅ | intellij:StringOperationCanBeSimplified, netbeans:Redundant String.toString() (performance), pmd:StringToString, errorprone:SubstringOfZero | `s.toString()` → `s; s.substring(0)` → `s` |  |
| S110 | Umständliche String-Operation | 2* | ⚠️ | picnic:StringRules.StringIsBlank+StringSubstring+StringIsEmptyWithString+StringsIsNullOrEmpty, pmd:InefficientEmptyStringCheck | `s.substring(i, s.length())` → `s.substring(i); s.trim().isEmpty()` → `s.isBlank()` |  |
| S111 | indexOf()-Vergleich statt contains() | 2* | ✅ | netbeans:DN_containsForIndexOf (rules15), intellij:ListIndexOfReplaceableByContains, intellij:IndexOfReplaceableByContains | `s.indexOf("e") >= 0` → `s.contains("e")` |  |
| S112 | equals("") statt isEmpty() | 2* | ✅ | intellij:StringEqualsEmptyString, netbeans:String.equals("") (performance) | `s.equals("")` → `s.isEmpty()` |  |
| S113 | size()/length() == 0 statt isEmpty() ★ | 2* | ✅ | baseline:CardinalityEqualsZero, intellij:SizeReplaceableByIsEmpty, netbeans:Usage of [Collection\|Map].size() == 0 (performance), pmd:UseCollectionIsEmpty, picnic:CharSequenceRules.CharSequenceIsEmpty | `list.size() == 0` → `list.isEmpty()` |  |
| S114 | intern() auf String-Literal | 1 | ✅ | netbeans:String.intern() called on constant (performance) | `"abc".intern()` → `"abc"` |  |
| S115 | Unnötiges Escape im String-Literal | 1 | ✅ | intellij:UnnecessaryStringEscape, picnic:RedundantStringEscape | `"it\'s"` → `"it's"` |  |
| S116 | Überflüssige Unicode-Escape-Sequenz | 1 | ✅ | errorprone:UnicodeEscape, intellij:UnnecessaryUnicodeEscape, intellij:ConvertToBasicLatin | `"A"` → `"A"` | nur druckbare Zeichen |
| S117 | Überflüssiges Escape im Regex-Ersetzungsstring | 2* | ✅ | intellij:RedundantEscapeInRegexReplacement | `s.replaceAll("a", "\\-")` → `s.replaceAll("a", "-")` |  |
| S118 | StringBuilder-Kette statt String-Verkettung | 2 | ✅ | baseline:StringBuilderConstantParameters, netbeans:Replace StringBuffer/StringBuilder by String (performance), errorprone:UnnecessaryStringBuilder, intellij:StringBufferReplaceableByString | `new StringBuilder().append(a).append(b).toString()` → `a + b` |  |
| S119 | String.concat() statt + | 2* | ⚠️ | intellij:CallToStringConcatCanBeReplacedByOperator | `s.concat(t)` → `s + t` |  |
| S120 | String.format nur mit Literalen | 2 | ✅ | errorprone:StringFormatWithLiteral | `String.format("%s!", "hi")` → `"hi!"` |  |
| S121 | Überflüssiger String.format-Aufruf | 1 | ✅ | intellij:RedundantStringFormatCall | `System.out.println(String.format("n: %d", n))` → `System.out.printf("n: %d%n", n)` |  |
| S122 | Formatstring-Konstante nur einmal verwendet | 2 | ✅ | errorprone:InlineFormatString | `static final String F = "%s"; String.format(F, x)` → `String.format("%s", x)` |  |
| S123 | String.format nur mit Trennzeichen statt String.join | 2* | ⚠️ | picnic:StringJoining | `String.format("%s,%s", a, b)` → `String.join(",", a, b)` |  |
| S124 | Unnötiges Wrapper-Objekt bei Konvertierung von/zu String | 1 | ✅ | intellij:UnnecessaryTemporaryOnConversionToString, intellij:UnnecessaryTemporaryOnConversionFromString, pmd:UnnecessaryConversionTemporary, netbeans:Unnecessary temporary during conversion from String (performance), netbeans:Unnecessary temporary during conversion to String (performance) | `new Integer(x).toString()` → `Integer.toString(x); new Integer("3").intValue()` → `Integer.parseInt("3")` | ergänzt S019 |
| S125 | System.getProperty für Werte mit eigener API | 1 | ✅ | intellij:SystemGetProperty | `System.getProperty("line.separator")` → `System.lineSeparator()` |  |
| S126 | Instanz nur für getClass() erzeugt | 1 | ✅ | pmd:InstantiationToGetClass, netbeans:.getClass() replaceable with .class (performance), intellij:InstantiatingObjectToGetClassObject | `new Foo().getClass()` → `Foo.class` |  |
| S127 | Class-Methode auf Klassenliteral statt instanceof/Cast | 1 | ✅ | intellij:RedundantClassCall | `String.class.isInstance(o)` → `o instanceof String` |  |
| S128 | isAssignableFrom(o.getClass()) statt isInstance(o) | 1 | ✅ | picnic:ClassRules.ClassIsInstanceWithClassAndObject | `c.isAssignableFrom(o.getClass())` → `c.isInstance(o)` |  |
| S129 | Überflüssige Identitätskonvertierung ★ | 2* | ✅ | picnic:IdentityConversion, netbeans:Boxing of already boxed value (performance), intellij:BoxingBoxedValue | `String s; String.valueOf(s)` → `s` |  |
| S130 | Lambda durch Methodenreferenz ersetzbar | 2* | ✅ | intellij:Convert2MethodRef, pmd:LambdaCanBeMethodReference, baseline:LambdaMethodReference, netbeans:DN_lambda2MemberReference (suggestions), picnic:ClassRules.ClassIsInstance+ClassIsInstanceWithClass+ClassCast, picnic:StringRules.StringIsEmpty+NotStringIsEmpty+StringValueOf | `x -> foo(x)` → `this::foo; s -> s.isEmpty()` → `String::isEmpty` |  |
| S131 | instanceof-/Cast-Lambda statt Class::isInstance/cast | 1 | ✅ | picnic:ClassCastLambdaUsage, picnic:IsInstanceLambdaUsage | `x -> x instanceof Foo` → `Foo.class::isInstance` |  |
| S132 | Lambda durch JDK-Methode ersetzbar | 2* | ✅ | intellij:LambdaCanBeMethodCall | `Function<T, T> f = x -> x` → `Function.identity()` |  |
| S133 | Triviales Lambda statt Wert-Überladung (orElseGet) | 1 | ✅ | intellij:ExcessiveLambdaUsage, baseline:OptionalOrElseGetValue, picnic:OptionalRules.OptionalOrElse+OptionalOrElseThrow | `opt.orElseGet(() -> "x")` → `opt.orElse("x")` | nur Literale/Variablen im Lambda |
| S134 | Lambda sofort an Ort und Stelle aufgerufen | 1 | ✅ | intellij:TrivialFunctionalExpressionUsage | `((Supplier<String>) () -> "a").get()` → `"a"` |  |
| S135 | Array aller Enum-Konstanten statt values() | 2 | ⚠️ | intellij:ArrayCanBeReplacedWithEnumValues | `new Color[] {RED, GREEN, BLUE}` → `Color.values()` | nur Enums der Datei |
| S136 | Überflüssiges Array für Varargs-Parameter | 2* | ✅ | errorprone:ExplicitArrayForVarargs, pmd:UnnecessaryVarargsArrayCreation, intellij:RedundantArrayCreation | `Arrays.asList(new String[] {"a", "b"})` → `Arrays.asList("a", "b")` |  |
| S137 | new T[] im Array-Initialisierer überflüssig ★ | 1 | ✅ | intellij:UnnecessaryConstantArrayCreationExpression, pmd:UseShortArrayInitializer | `int[] a = new int[] {1, 2};` → `int[] a = {1, 2};` |  |
| S138 | Container erzeugt, nur ein Element gelesen | 2 | ✅ | intellij:OnlyOneElementUsed | `List.of(a, b).get(0)` → `a` |  |
| S139 | addAll() direkt nach leerem Konstruktor | 2 | ✅ | intellij:CollectionAddAllCanBeReplacedWithConstructor | `List<X> l = new ArrayList<>(); l.addAll(c);` → `new ArrayList<>(c)` |  |
| S140 | Schleife mit add/remove statt addAll/removeAll | 2* | ✅ | picnic:CollectionRules.CollectionAddAllBlock+CollectionRemoveAllBlock, intellij:ManualArrayToCollectionCopy, intellij:UseBulkOperation, pmd:UseArraysAsList | `for (String s : arr) list.add(s);` → `Collections.addAll(list, arr)` |  |
| S141 | Schleife statt Arrays.fill/setAll | 1 | ✅ | intellij:ExplicitArrayFilling | `for (int i = 0; i < a.length; i++) a[i] = 0;` → `Arrays.fill(a, 0)` |  |
| S142 | Array-Kopie per Schleife statt System.arraycopy | 1 | ✅ | intellij:ManualArrayCopy, netbeans:Manual array copy (performance), pmd:AvoidArrayLoops | `for (int i = 0; i < n; i++) b[i] = a[i];` → `System.arraycopy(a, 0, b, 0, n)` |  |
| S143 | Umständliche Collection-Operation mit einfacher Alternative | 2* | ✅ | intellij:RedundantCollectionOperation | `Arrays.asList(arr).contains(x) …` |  |
| S144 | Umweg über keySet()/values() statt Map-Methode | 1 | ✅ | picnic:MapRules.MapGet+MapIsEmpty+MapSize+MapContainsKey+MapContainsValue+MapKeySetStream+MapValuesStream | `m.keySet().contains(k)` → `m.containsKey(k)` |  |
| S145 | Unmodifiable-Wrapper um unveränderliche Collection | 2 | ✅ | intellij:RedundantUnmodifiable | `Collections.unmodifiableList(List.of(a))` → `List.of(a)` |  |
| S146 | Guava: Builder-Konstruktor statt Fabrikmethode | 2 | ✅ | picnic:ImmutableListMultimapRules.ImmutableListMultimapBuilder, picnic:ImmutableListRules.ImmutableListBuilder, picnic:ImmutableMapRules.ImmutableMapBuilder+BuilderBuildOrThrow, picnic:ImmutableMultisetRules.ImmutableMultisetBuilder, picnic:ImmutableSetMultimapRules.ImmutableSetMultimapBuilder, picnic:ImmutableSetRules.ImmutableSetBuilder, picnic:ImmutableSortedMapRules.ImmutableSortedMapOrderedBy+ImmutableSortedMapNaturalOrder+ImmutableSortedMapReverseOrder, picnic:ImmutableSortedMultisetRules.ImmutableSortedMultisetOrderedBy+ImmutableSortedMultisetNaturalOrder+ImmutableSortedMultisetReverseOrder, picnic:ImmutableSortedSetRules.ImmutableSortedSetOrderedBy+ImmutableSortedSetNaturalOrder+ImmutableSortedSetReverseOrder, picnic:ImmutableTableRules.ImmutableTableBuilder+BuilderBuildOrThrow | `new ImmutableList.Builder<>()` → `ImmutableList.builder()` | build() → buildOrThrow() nur 2* |
| S147 | Vereinfachbare Stream-Aufrufkette | 2* | ✅ | intellij:SimplifyStreamApiCallChains, picnic:DoubleStreamRules.DoubleStreamFilterSorted+DoubleStreamFindAnyIsEmpty+DoubleStreamFindAnyIsPresent+DoubleStreamMin+DoubleStreamNoneMatchWithDoublePredicate+DoubleStreamAnyMatch+DoubleStreamAllMatchWithDoublePredicate+DoubleStreamTakeWhile, picnic:IntStreamRules.IntStreamFilterSorted+IntStreamFindAnyIsEmpty+IntStreamFindAnyIsPresent+IntStreamMin+IntStreamNoneMatchWithIntPredicate+IntStreamAnyMatch+IntStreamAllMatchWithIntPredicate+IntStreamTakeWhile, picnic:LongStreamRules.LongStreamFilterSorted+LongStreamFindAnyIsEmpty+LongStreamFindAnyIsPresent+LongStreamMin+LongStreamNoneMatchWithLongPredicate+LongStreamAnyMatch+LongStreamAllMatchWithLongPredicate+LongStreamTakeWhile, picnic:StreamRules.StreamFilterSorted+StreamFilterSortedWithComparator+StreamDistinctSorted+StreamDistinctSortedWithComparator+StreamFindAnyIsEmpty+StreamFindAnyIsPresent+StreamMin+StreamMinNaturalOrder+StreamMax+StreamMaxNaturalOrder+StreamNoneMatchWithPredicate+StreamAnyMatch+StreamAllMatchWithPredicate+StreamTakeWhile+StreamFindFirst+StreamFindFirstMap, baseline:CollectionStreamForEach, picnic:CollectionRules.CollectionForEach | `s.filter(p).findAny().isPresent()` → `s.anyMatch(p); c.stream().forEach(f)` → `c.forEach(f)` | Umordnungen (sorted/filter) unsafe |
| S148 | Überflüssiger Stream-/Optional-Aufruf | 2* | ✅ | intellij:RedundantStreamOptionalCall | `stream.map(x -> x)` → `stream` |  |
| S149 | Umständliche Stream-Erzeugung | 2 | ✅ | picnic:StreamRules.StreamOfNullable+StreamIterate+CollectionsNCopiesStream+StreamOf1+StreamOf2+StreamOf3+StreamOf4+StreamOf5 | `Stream.iterate(s, f).takeWhile(p)` → `Stream.iterate(s, p, f)` |  |
| S150 | Stream.of() statt Stream.empty() | 2 | ✅ | baseline:StreamOfEmpty, picnic:StreamRules.StreamEmpty | `Stream.of()` → `Stream.empty()` |  |
| S151 | Mehrfache parallel()/sequential()-Aufrufe | 1 | ✅ | errorprone:MultipleParallelOrSequentialCalls | `s.parallel().map(f).parallel()` → `s.parallel().map(f)` |  |
| S152 | forEach durch passendere Stream-Operation ersetzbar | 2* | ⚠️ | intellij:SimplifyForEach | `stream.forEach(x -> list.add(x))` → `collect(toList())` |  |
| S153 | Nachgelagerte Transformation in Stream-Kette ziehbar | 2 | ⚠️ | intellij:FuseStreamOperations | `new HashSet<>(s.collect(toList()))` → `s.collect(toSet())` |  |
| S154 | Vereinfachbarer Collector | 2* | ✅ | intellij:SimplifyCollector | `collect(groupingBy(f, collectingAndThen(…)))` → `toMap(…)` |  |
| S155 | Collectors.toList/toSet ohne festgelegte Veränderlichkeit | 2 | ⚠️ | picnic:CollectorMutability | `collect(Collectors.toList())` → `toList() / toCollection(ArrayList::new)` |  |
| S156 | rangeClosed(a, b - 1) statt range(a, b) | 2 | ⚠️ | picnic:IntStreamRules.IntStreamRange, picnic:LongStreamRules.LongStreamRange | `IntStream.rangeClosed(0, n - 1)` → `IntStream.range(0, n)` | unsafe: Überlauf bei MIN_VALUE |
| S157 | Überflüssiges Standardargument (naturalOrder(), joining("")) | 2 | ✅ | picnic:CollectionRules.CollectionsSort+CollectionsMin+CollectionsMax, picnic:StreamRules.Joining+StreamSorted | `Collections.sort(l, naturalOrder())` → `Collections.sort(l); joining("")` → `joining()` |  |
| S158 | Umständlich formulierte natürliche/umgekehrte Ordnung | 2 | ✅ | picnic:ComparatorRules.NaturalOrder+ReverseOrder | `Collections.reverseOrder()` → `Comparator.reverseOrder()` |  |
| S159 | Umgekehrte Ordnung mit gegenteiliger Operation | 2 | ✅ | picnic:ComparatorRules.MinByNaturalOrder+MaxByNaturalOrder+LeastNaturalOrder+GreatestNaturalOrder | `maxBy(reverseOrder())` → `minBy(naturalOrder())` |  |
| S160 | thenComparing(comparing(f)) statt thenComparing(f) | 2 | ✅ | intellij:RedundantComparatorComparing, picnic:ComparatorRules.ComparatorThenComparing+ComparatorThenComparingReverseOrder+ComparatorThenComparingWithComparator+ComparatorThenComparingComparatorReversed+ComparatorThenComparingDouble+ComparatorThenComparingInt+ComparatorThenComparingLong | `c.thenComparing(comparing(f))` → `c.thenComparing(f)` |  |
| S161 | comparing(Map.Entry::getKey) statt comparingByKey() | 1 | ✅ | picnic:MapEntryRules.ComparingByKey+ComparingByKeyWithComparator+ComparingByValue+ComparingByValueWithComparator | `comparing(Map.Entry::getKey)` → `Map.Entry.comparingByKey()` |  |
| S162 | Vereinfachbare Optional-Aufrufkette | 2* | ✅ | intellij:SimplifyOptionalCallChains | `opt.map(x -> true).orElse(false)` → `opt.isPresent()` |  |
| S163 | Umständliche Optional-Erzeugung | 1 | ✅ | picnic:OptionalRules.OptionalEmpty+OptionalOfNullable | `x == null ? Optional.empty() : Optional.of(x)` → `Optional.ofNullable(x)` |  |
| S164 | flatMap mit Optional.ofNullable statt map | 2* | ✅ | baseline:OptionalFlatMapOfNullable | `opt.flatMap(x -> Optional.ofNullable(f(x)))` → `opt.map(x -> f(x))` |  |
| S165 | opt.get() im Lambda statt Parameter | 2* | ✅ | errorprone:UnnecessaryOptionalGet | `opt.map(x -> opt.get().f())` → `opt.map(x -> x.f())` |  |
| S166 | isPresent()/get() statt ifPresent/map/orElse | 2* | ⚠️ | intellij:OptionalIsPresent | `if (o.isPresent()) use(o.get());` → `o.ifPresent(this::use);` |  |
| S167 | Optional.of(...) mit überflüssiger Prüfung | 2 | ⚠️ | errorprone:OptionalOfRedundantMethod | `Optional.of(x).isPresent()` → `true` |  |
| S168 | if/throw statt Preconditions.checkArgument/checkState | 1 | ⚠️ | errorprone:PreferPreconditions, picnic:PreconditionsRules.CheckArgumentNot+CheckArgumentNotWithString+CheckStateNot+CheckStateNotWithString+CheckElementIndex+CheckPositionIndex+CheckPositionIndexWithString | `if (x < 0) throw new IllegalArgumentException();` → `checkArgument(x >= 0)` | nur wenn Guava importiert |
| S169 | Explizite Null-Prüfung statt Objects.requireNonNull | 1 | ⚠️ | errorprone:RequireNonNullRefactoring | `if (x == null) throw new NullPointerException();` → `Objects.requireNonNull(x)` |  |
| S170 | checkNotNull-Meldung wiederholt das Argument | 2 | ✅ | errorprone:PreconditionsCheckNotNullRepeated | `checkNotNull(x, "%s", x)` → `checkNotNull(x, "x")` |  |
| S171 | Duration in unpassender Einheit | 2 | ✅ | errorprone:CanonicalDuration | `Duration.ofSeconds(60)` → `Duration.ofMinutes(1)` |  |
| S172 | Duration.of(n, ChronoUnit.X) statt Duration.ofX(n) | 2 | ✅ | picnic:TimeRules.DurationOfDays+DurationOfHours+DurationOfMillis+DurationOfMinutes+DurationOfNanos+DurationOfSeconds | `Duration.of(5, ChronoUnit.SECONDS)` → `Duration.ofSeconds(5)` |  |
| S173 | Fabrikaufruf statt Zeit-Konstante (UTC, ZERO, EPOCH) | 2 | ✅ | picnic:TimeRules.Utc+ClockSystemUTC+InstantEpoch+LocalTimeMin+DurationZero+PeriodZero, baseline:ZoneIdConstant, errorprone:ZoneIdOfZ | `ZoneId.of("Z")` → `ZoneOffset.UTC; Duration.ofSeconds(0)` → `Duration.ZERO` | ZoneId.of("UTC") → ZoneOffset.UTC ist unsafe (ZoneRegion ≠ ZoneOffset) |
| S174 | Überflüssige java.time-Umwandlung | 2* | ✅ | intellij:RedundantJavaTimeOperations | `LocalDateTime.of(d, t).toLocalDate()` → `d` |  |
| S175 | Überflüssiges UTF_8-Argument bei Files | 2 | ✅ | picnic:FileRules.FilesReadStringWithCharset+FilesReadString+FilesReadAllLines+FilesLines+FilesWriteString+FilesWrite+FilesNewBufferedWriter | `Files.readString(p, UTF_8)` → `Files.readString(p)` | ab 11 |
| S176 | new File(path) als Argument überflüssig | 2 | ✅ | intellij:RedundantFileCreation | `new FileInputStream(new File(path))` → `new FileInputStream(path)` |  |
| S177 | Listener mit leeren Methoden statt Adapter-Klasse | 2 | ⚠️ | intellij:ListenerMayUseAdapter | `class L implements MouseListener { … }` → `extends MouseAdapter` |  |
| S178 | Gleicher Ausdruck mehrfach in einer Methode | 2* | – | intellij:DuplicateExpressions | `a.length() * 2 … a.length() * 2` |  |
| S179 | Verwendung einer Void-Variable | 2 | ✅ | errorprone:VoidUsed | `Void v; f(v)` → `f(null)` |  |
| S180 | else-if-Bedingung ist die Negation der if-Bedingung | 1 | ✅ | eclipse-cleanup:REDUNDANT_IF_CONDITION | `if (c) a(); else if (!c) b();` → `if (c) a(); else b();` | nur bei seiteneffektfreier Bedingung |
| S181 | Gleiche Anweisungen am Ende aller Zweige | 1 | ✅ | eclipse-cleanup:CONTROLFLOW_MERGE | `if (c) { a(); x(); } else { b(); x(); }` → `if (c) a(); else b(); x();` | nur mit else-Zweig; ganze identische Zweige meldet B197 |
| S182 | Gleiche innere Bedingung in if- und else-Zweig | 1 | ⚠️ | eclipse-cleanup:PULL_OUT_IF_FROM_IF_ELSE | `if (a) { if (b) x(); } else { if (b) y(); }` → `if (b) { if (a) x(); else y(); }` | ändert die Auswertungsreihenfolge von a und b |
| S183 | Zweig mit Sprung wiederholt den folgenden Code | 1 | ✅ | eclipse-cleanup:REDUNDANT_FALLING_THROUGH_BLOCK_END | `if (c) { f(); return; } f(); return;` → `f(); return;` | nur bei seiteneffektfreier Bedingung |
| S184 | Aufeinanderfolgende if mit gleichem Sprung-Block | 1 | ✅ | eclipse-cleanup:ONE_IF_RATHER_THAN_DUPLICATE_BLOCKS_THAT_FALL_THROUGH, eclipse-cleanup:MERGE_CONDITIONAL_BLOCKS | `if (a) return x; if (b) return x;` → `if (a \|\| b) return x;` | Blöcke müssen mit return/throw/break/continue enden |
| S185 | Wertprüfung vor gleichwertigem return oder Zuweisung überflüssig | 1 | ✅ | eclipse-cleanup:REMOVE_REDUNDANT_COMPARISON_STATEMENT | `if (i == 0) return 0; return i;` → `return i;` | nur == mit Literal oder Konstante |
| S186 | compareTo auf Wrapper statt statischem compare | 1 | ✅ | eclipse-cleanup:PRIMITIVE_COMPARISON | `Integer.valueOf(a).compareTo(b)` → `Integer.compare(a, b)` |  |
| S187 | getClass() == X.class statt instanceof bei finaler Klasse | 2 | ⚠️ | eclipse-cleanup:INSTANCEOF | `o.getClass() == String.class` → `o instanceof String` | nur für final-Klassen der Datei oder des JDK; bei o == null wirft getClass(), instanceof nicht |

### M: Modernisierung

| Code | Regel | Stufe | Fix | Herkunft | Beispiel | Hinweis |
|---|---|---|---|---|---|---|
| M017 | Anonyme Klasse durch Lambda oder Methodenreferenz ersetzbar | 2* | ⚠️ | intellij:Convert2Lambda, javac:compiler.warn.potential.lambda.found, intellij:Anonymous2MethodRef, netbeans:Convert to Lambda or Member Reference (rules15) | `new Runnable() { public void run() { f(); } }` → `() -> f() bzw. this::f` | ab 8; nur bekannte JDK-Funktionsinterfaces bzw. Interfaces der Datei; this/Shadowing beachten |
| M018 | Anonyme Klasse durch Fabrikmethode mit Lambda ersetzbar | 2 | ⚠️ | intellij:AnonymousHasLambdaAlternative | `new ThreadLocal<>() { protected T initialValue() { return x; } }` → `ThreadLocal.withInitial(() -> x)` | ab 8 |
| M019 | Klasse kann Record sein | 2 | ⚠️ | intellij:ClassCanBeRecord | `final class P { private final int x; … getX() }` → `record P(int x)` | ab 16; Accessor-Namen ändern sich |
| M020 | Record-Pattern statt Accessor-Aufrufen (auch verschachtelt) | 2* | ✅ | netbeans:DN_ConvertToRecordPattern (rules15), intellij:DeconstructionCanBeUsed, netbeans:DN_ConvertToNestedRecordPattern (rules15) | `if (o instanceof Point p) { p.x() + p.y() }` → `o instanceof Point(int x, int y)` | ab 21; nur Records der Datei |
| M021 | if-else-Kette durch switch ersetzbar (String, Enum, instanceof) | 2* | ⚠️ | errorprone:UseEnumSwitch, intellij:IfCanBeSwitch, errorprone:IfChainToSwitch, netbeans:DN_ConvertToSwitchPatternInstanceOf (rules15), netbeans:Use switch over Strings where possible. (rules15), intellij:InstanceofChain | `if ("a".equals(s)) … else if ("b".equals(s)) …` → `switch (s)` | ab 7 (String), ab 21 (Pattern-switch); String/instanceof Stufe 1, Enum nur bei lokal bekanntem Typ |
| M022 | Iterator-Schleife durch enhanced for ersetzbar | 2 | ⚠️ | netbeans:DN_IteratorToFor (rules15), errorprone:ForEachIterable, intellij:WhileCanBeForeach | `for (Iterator<T> it = l.iterator(); it.hasNext();) { T t = it.next(); }` → `for (T t : l)` | ab 5; nur wenn it sonst nicht verwendet wird |
| M023 | try/finally mit close() statt try-with-resources | 1 | ⚠️ | eclipse:COMPILER_PB_EXPLICITLY_CLOSED_AUTOCLOSEABLE, intellij:TryFinallyCanBeTryWithResources, pmd:UseTryWithResources, netbeans:Convert to try-with-resources (rules15) | `InputStream in = …; try { … } finally { in.close(); }` → `try (InputStream in = …) { … }` | ab 7 (try (in) ab 9) |
| M024 | Überflüssige Variable in try-with-resources | 1 | ✅ | errorprone:TryWithResourcesVariable | `try (Foo f2 = f)` → `try (f)` | ab 9; f effektiv final |
| M025 | Serialisierungs-Member ohne @Serial | 2 | ✅ | intellij:MissingSerialAnnotation | `private static final long serialVersionUID = 1L;` → `@Serial private static final long serialVersionUID = 1L;` | ab 14 |
| M026 | Member eines @Deprecated-Typs nicht als deprecated markiert | 1 | ✅ | eclipse:COMPILER_PB_MEMBER_OF_DEPRECATED_TYPE | `@Deprecated class A { public void f() {} }` | ergänzt M013 |
| M027 | System.out.println statt IO.println | 1 | ✅ | intellij:MigrateToJavaLangIo | `System.out.println(x)` → `IO.println(x)` | ab 25; nur Compact Source Files sinnvoll |
| M028 | Iterator-Schleife mit remove() statt removeIf | 1 | ✅ | intellij:Java8CollectionRemoveIf | `for (Iterator<X> it = c.iterator(); it.hasNext();) if (p(it.next())) it.remove();` → `c.removeIf(…)` | ab 8 |
| M029 | Index-Schleife mit set() statt List.replaceAll | 2* | ✅ | intellij:Java8ListReplaceAll | `for (i…) l.set(i, l.get(i).trim());` → `l.replaceAll(String::trim)` | ab 8 |
| M030 | containsKey/get/put-Muster statt Map-Default-Methoden | 2* | ⚠️ | intellij:Java8MapApi | `if (!m.containsKey(k)) m.put(k, v);` → `m.putIfAbsent(k, v);` | ab 8; null-Werte ändern die Semantik |
| M031 | Collections.sort(list) statt list.sort | 2 | ✅ | intellij:Java8ListSort, modernizer:Collections.sort → List.sort | `Collections.sort(list, cmp)` → `list.sort(cmp)` | ab 8 |
| M032 | Comparator-Lambda statt Comparator.comparing | 2* | ⚠️ | intellij:ComparatorCombinators | `(a, b) -> a.getName().compareTo(b.getName())` → `Comparator.comparing(P::getName)` | ab 8 |
| M033 | compare-Bedingung statt Comparator.max/min | 2* | ✅ | intellij:ComparatorMinMaxCanBeUsed | `comp.compare(a, b) >= 0 ? a : b` → `comp.max(a, b)` | ab 26 |
| M034 | Indexzugriff statt getFirst/getLast/removeLast | 2* | ⚠️ | intellij:SequencedCollectionMethodCanBeUsed, picnic:CollectionRules.SequencedCollectionGetFirst+SequencedCollectionGetLast+ListAddFirst+ListAdd+ListRemoveFirst+ListRemoveLast | `list.get(list.size() - 1)` → `list.getLast()` | ab 21; leere Liste wirft andere Exception |
| M035 | Rohe Konstante Collections.EMPTY_LIST statt emptyList() | 2 | ✅ | intellij:CollectionsFieldAccessReplaceableByMethodCall | `Collections.EMPTY_LIST` → `Collections.emptyList()` | ab 5 |
| M036 | Collections.empty*/singleton*/unmodifiable* statt List.of/Set.of/Map.of | 2 | ⚠️ | modernizer:Collections.empty*/singleton*/EMPTY_* → List.of/Set.of/Map.of, intellij:Java9CollectionFactory | `Collections.unmodifiableList(Arrays.asList(a, b))` → `List.of(a, b)` | ab 9; List.of verbietet null; Signaturliste |
| M037 | Veraltete Collection-Klassen (Vector, Hashtable, Stack, Enumeration …) | 2 | ⚠️ | intellij:EnumerationCanBeIteration, pmd:ReplaceEnumerationWithIterator, pmd:ReplaceHashtableWithMap, pmd:ReplaceJavaUtilVector, modernizer:Vector/Hashtable/Stack/Dictionary/Enumeration/StringTokenizer, netbeans:Obsolete Collection (code_maturity), errorprone:JdkObsolete, intellij:ObsoleteCollection | `new Vector<>()` → `new ArrayList<>(); v.elements()` → `v.iterator()` | Klassenliste als Daten; Synchronisation geht verloren |
| M038 | String.format statt formatted | 2 | ✅ | modernizer:String.format → formatted, picnic:StringRules.Formatted | `String.format("x %s", a)` → `"x %s".formatted(a)` | ab 15; nur mit Literal als Format |
| M039 | Schleife statt String.repeat/StringBuilder.repeat | 2* | ✅ | intellij:StringRepeatCanBeUsed | `for (int i = 0; i < n; i++) sb.append(" ");` → `sb.append(" ".repeat(n))` | ab 11 (StringBuilder.repeat ab 21) |
| M040 | append(s.repeat(n)) statt StringBuilder.repeat | 2* | ✅ | picnic:StringBuilderRules.StringBuilderRepeat | `sb.append("-".repeat(n))` → `sb.repeat("-", n)` | ab 21 |
| M041 | Charset als Name, Plattform-Default oder Bibliothekskonstante statt StandardCharsets ★ | 2 | ✅ | pmd:UseStandardCharsets, modernizer:Charset-Name/Default-Charset → Charset-Overloads, intellij:CharsetObjectCanBeUsed, modernizer:Charsets-Konstanten → StandardCharsets, errorprone:StringCharset, errorprone:PreferCharsetOverload | `s.getBytes("UTF-8")` → `s.getBytes(StandardCharsets.UTF_8); Charset.forName("UTF-8")` → `UTF_8` | ab 7 (Charset-Overloads teils ab 10); Signaturliste; Fix unsafe für Default-Charset-Overloads |
| M042 | Bytes lesen und new String statt Files.readString/writeString | 2 | ⚠️ | intellij:ReadWriteStringCanBeUsed | `new String(Files.readAllBytes(p), UTF_8)` → `Files.readString(p)` | ab 11; readString wirft bei ungültigen Bytes |
| M043 | Paths.get statt Path.of ★ | 2 | ✅ | modernizer:Paths.get → Path.of, picnic:FileRules.PathOf+PathOfVarargs | `Paths.get("a", "b")` → `Path.of("a", "b")` | ab 11 |
| M044 | java.io.File-API statt java.nio.file | 2 | ⚠️ | modernizer:java.io.File/FileInputStream/FileReader/FileWriter → java.nio.file, picnic:FileRules.FilesCreateTempFileToFile+FilesCreateTempFileFileToPathToFile | `new File(s)` → `Path.of(s); File.createTempFile(..)` → `Files.createTempFile(..)` | ab 7; Signaturliste |
| M045 | Optional.get()/!isPresent() statt orElseThrow()/isEmpty() | 2* | ✅ | modernizer:Optional.get/getAsXxx → orElseThrow, picnic:OptionalRules.OptionalOrElseThrowWithOptional+OptionalIsEmpty+OptionalIsPresent | `opt.get()` → `opt.orElseThrow(); !opt.isPresent()` → `opt.isEmpty()` | ab 10 (isEmpty ab 11) |
| M046 | Null-Prüfung/Optional.ofNullable statt Objects.requireNonNullElse | 2 | ⚠️ | intellij:ReplaceNullCheck, picnic:NullRules.RequireNonNullElse+RequireNonNullElseGet | `x != null ? x : def` → `Objects.requireNonNullElse(x, def)` | ab 9; wirft, wenn def null ist |
| M047 | Manuelle Indexprüfung statt Objects.checkIndex | 2 | ⚠️ | picnic:AssortedRules.CheckIndexExpression+CheckIndexBlock | `if (i < 0 \|\| i >= n) throw new IndexOutOfBoundsException();` → `Objects.checkIndex(i, n)` | ab 9 |
| M048 | Math.min(Math.max(…)) statt Math.clamp | 1 | ⚠️ | intellij:MathClampMigration, picnic:PrimitiveRules.MathClampInt+MathClampLong+MathClampFloat+MathClampDouble | `Math.max(Math.min(max, x), min)` → `Math.clamp(x, min, max)` | ab 21; clamp wirft bei min > max |
| M049 | Veraltete Datums-API (Date, Calendar, TimeZone) statt java.time | 2 | – | errorprone:JavaUtilDate, pmd:ReplaceJavaUtilCalendar, intellij:UseOfObsoleteDateTimeApi, intellij:UseOfObsoleteDateTimeApiInspection, pmd:ReplaceJavaUtilDate | `Date d = new Date();` → `Instant.now()` | ab 8 |
| M050 | Deprecated JDK-API bzw. Methode/Konstruktor mit modernem Nachfolger | 2* | ⚠️ | intellij:BigDecimalLegacyMethod, errorprone:ClassNewInstance, intellij:ClassNewInstance, modernizer:Konstruktor → Fabrikmethode (Locale.of, Reader.of, URI.toURL, InetAddress.ofLiteral), errorprone:JavaDurationGetSecondsToToSeconds, baseline:InvocationTargetExceptionGetTargetException, forbidden-apis:jdk-deprecated, modernizer:Veraltete JDK-Methoden mit Nachfolger (Thread.getId, Class.newInstance, BigDecimal-Rundung, Runtime.exec(String) …) | `Foo.class.newInstance()` → `getDeclaredConstructor().newInstance(); new Locale(s)` → `Locale.of(s); d.getSeconds()` → `d.toSeconds()` | Signaturliste je Java-Version (ab 2–27), Ersatz je Eintrag; Fix nur wo Nachfolger bekannt |
| M051 | Guava-Collection-Fabrik/-Hilfe statt JDK | 2 | ✅ | baseline:PreferCollectionConstructors, modernizer:Guava Collections-Fabriken → Konstruktoren, picnic:CollectionRules.CollectionAddAllExpression+CollectionRemoveAllExpression+NewArrayList, modernizer:Guava Collections-Hilfen → JDK (Lists.reverse, Maps.immutableEntry, Sets.newSetFromMap, unmodifiableNavigable*), baseline:PreferBuiltInConcurrentKeySet, picnic:MapEntryRules.MapEntry | `Lists.newArrayList()` → `new ArrayList<>(); Maps.immutableEntry(k, v)` → `Map.entry(k, v)` | ab 7 (Map.entry ab 9, reversed ab 21); Signaturliste; einzelne Einträge unsafe |
| M052 | Guava-Funktionstypen, Optional und Ordering statt java.util.function/Optional/Comparator | 2 | ⚠️ | intellij:StaticPseudoFunctionalStyleMethod, modernizer:Guava-Funktionsinterfaces → java.util.function, intellij:Guava, modernizer:Guava Optional → java.util.Optional, modernizer:Comparators/Ordering → Comparator, picnic:PatternRules.PatternAsPredicate+PatternCompileAsPredicate | `Iterables.transform(l, f)` → `l.stream().map(f); Ordering.natural()` → `Comparator.naturalOrder()` | ab 8; Signaturliste |
| M053 | Guava Objects/Preconditions/MoreObjects statt java.util.Objects | 2 | ✅ | picnic:PreconditionsRules.RequireNonNullExpression+RequireNonNullWithStringExpression+RequireNonNullWithStringBlock, baseline:DeprecatedGuavaObjects, modernizer:Guava Objects/MoreObjects/Preconditions → java.util.Objects | `Objects.equal(a, b)` → `java.util.Objects.equals(a, b); checkNotNull(x)` → `requireNonNull(x)` | ab 7 (requireNonNullElse ab 9); Signaturliste |
| M054 | Guava-Primitives-/Math-Hilfe statt JDK | 2 | ✅ | modernizer:Guava Primitives/IntMath/LongMath → JDK, picnic:PrimitiveRules.MathToIntExact+CharacterBytes+ShortBytes+IntegerBytes+LongBytes+FloatBytes+DoubleBytes+IntegerCompareUnsigned+LongCompareUnsigned+IntegerDivideUnsigned+LongDivideUnsigned+IntegerRemainderUnsigned+LongRemainderUnsigned+IntegerParseUnsignedInt+LongParseUnsignedLong+IntegerParseUnsignedIntWithInt+LongParseUnsignedLongWithInt+IntegerToUnsignedString+LongToUnsignedString+IntegerToUnsignedStringWithInt+LongToUnsignedStringWithInt+ArraysCompareUnsignedByte+ArraysCompareUnsignedInt+ArraysCompareUnsignedLong | `Ints.compare(a, b)` → `Integer.compare(a, b); Ints.checkedCast(x)` → `Math.toIntExact(x)` | ab 4–21 je Eintrag; Signaturliste |
| M055 | IO-Hilfsbibliothek (Guava, Commons IO) statt JDK | 2 | ⚠️ | modernizer:Commons IO (FileUtils, IOUtils, Null*Stream) → JDK, modernizer:Guava IO (Files, ByteStreams, CharStreams) → JDK, baseline:PreferInputStreamTransferTo, picnic:InputStreamRules.InputStreamTransferTo+InputStreamReadAllBytes+InputStreamReadNBytes+InputStreamSkipNBytes | `ByteStreams.copy(in, out)` → `in.transferTo(out); IOUtils.toByteArray(in)` → `in.readAllBytes()` | ab 7–25 je Eintrag (transferTo ab 9); Signaturliste |
| M056 | Commons Lang/Collections/Codec statt JDK | 2 | ⚠️ | modernizer:Base64/Hex-Codecs → java.util.Base64/HexFormat, modernizer:Commons Collections → JDK (Predicate/Transformer/Factory/Closure, ComparatorUtils, IteratorUtils), modernizer:Commons Lang → JDK (ObjectUtils, Validate, NumberUtils, CharUtils, SystemUtils, FastDateFormat) | `Validate.notNull(x)` → `Objects.requireNonNull(x); Base64.encodeBase64String(b)` → `Base64.getEncoder().encodeToString(b)` | ab 7–17 je Eintrag (HexFormat ab 17); Signaturliste |
| M057 | Joiner/StringUtils.join/repeat statt String.join/repeat | 2 | ⚠️ | modernizer:Joiner/StringUtils.join/repeat → String.join/repeat, picnic:StringRules.StringJoin+StringJoinVarargs+StringValueOfWithObject+NewString3+NewString1 | `Joiner.on(",").join(l)` → `String.join(",", l); StringUtils.repeat(s, n)` → `s.repeat(n)` | ab 8 (repeat ab 11); Signaturliste |
| M058 | Stream-/Optional-Hilfsmethode statt JDK-Methode | 2 | ⚠️ | modernizer:Stream-/Optional-Hilfen → JDK (Streams.stream(Optional), Iterators.forEnumeration, Collectors.toUnmodifiableList) | `Streams.stream(opt)` → `opt.stream(); collect(Collectors.toUnmodifiableList())` → `toList()` | ab 9/16; Signaturliste |
| M059 | Joda-Time statt java.time | 2 | ⚠️ | modernizer:Joda-Time → java.time | `DateTime.now()` → `ZonedDateTime.now()` | ab 8; Signaturliste |
| M060 | Schleife baut String mit Trennzeichen statt String.join | 2* | ⚠️ | eclipse-cleanup:JOIN | `for (String s : l) { if (sb.length() > 0) sb.append(","); sb.append(s); }` → `String.join(",", l)` | ab 8; nur für lokal als String-Collection oder String[] bekannte Quellen |
| M061 | Viele Imports eines Moduls durch import module ersetzbar | 2 | ⚠️ | eclipse-cleanup:USE_MODULE_IMPORTS | `import java.util.List; import java.util.Map; …` → `import module java.base;` | ab 25; Option, nicht im Standard; Namenskonflikte (List in java.awt) möglich |

### P: Performance (neu)

| Code | Regel | Stufe | Fix | Herkunft | Beispiel | Hinweis |
|---|---|---|---|---|---|---|
| P001 | Einbuchstabiger String statt char (append, indexOf) | 2* | ✅ | pmd:AppendCharacterWithChar, intellij:LengthOneStringInIndexOf, pmd:UseIndexOfChar, netbeans:Length one String in String.indexOf (performance) | `s.indexOf("x")` → `s.indexOf('x'); sb.append("a")` → `sb.append('a')` |  |
| P002 | String-Verkettung im Argument von append() | 2* | ✅ | pmd:InefficientStringBuffering, intellij:StringConcatenationInsideStringBufferAppend, netbeans:String concatenation in StringBuilder.append (performance) | `sb.append("a" + b)` → `sb.append("a").append(b)` |  |
| P003 | Aufeinanderfolgende append ohne Verkettung | 2* | ✅ | pmd:ConsecutiveAppendsShouldReuse | `sb.append(a); sb.append(b);` → `sb.append(a).append(b);` |  |
| P004 | Mehrere Literal-appends hintereinander | 2* | ✅ | pmd:ConsecutiveLiteralAppends | `sb.append("a").append("b")` → `sb.append("ab")` |  |
| P005 | toString().length() auf StringBuilder | 2* | ✅ | pmd:UseStringBufferLength | `sb.toString().length()` → `sb.length()` |  |
| P006 | StringBuilder/StringBuffer als Feld | 2 | – | pmd:AvoidStringBufferField, intellij:StringBufferField | `private StringBuilder sb = new StringBuilder();` |  |
| P007 | String in Schleife bzw. wiederholt mit += verkettet | 2* | – | intellij:StringConcatenationInLoops, pmd:UseStringBufferForStringAppends, intellij:StringReplaceableByStringBuffer | `for (…) { s += x; }` → `StringBuilder` |  |
| P008 | toUpperCase/toLowerCase vor equals | 1 | ⚠️ | pmd:UnnecessaryCaseChange | `s.toUpperCase().equals("A")` → `s.equalsIgnoreCase("A")` | Locale-Randfälle |
| P009 | replaceAll mit Literal ohne Regex-Zeichen statt replace | 2* | ✅ | intellij:ReplaceAllNonRegex | `s.replaceAll("abc", "x")` → `s.replace("abc", "x")` | Ersatz darf kein $ oder \ enthalten |
| P010 | Konstanter Regex bei jedem Aufruf neu kompiliert | 2* | ⚠️ | errorprone:ConstantPatternCompile, intellij:DynamicRegexReplaceableByCompiledPattern | `s.matches("\\d+") / Pattern.compile("a+") in Methode` → `static final Pattern` |  |
| P011 | String.intern() verwendet | 2* | – | baseline:DangerousStringInternUsage | `s.intern()` |  |
| P012 | Lokale Wrapper-Variable kann primitiv sein | 2 | ⚠️ | errorprone:UnnecessaryBoxedVariable, intellij:WrapperTypeMayBePrimitive | `Integer n = 0; n += 1;` → `int n = 0;` | nur ohne null-Zuweisung und ohne Verwendung als Objekt |
| P013 | Objects.equals mit primitiven Werten | 2* | ✅ | errorprone:ObjectEqualsForPrimitives | `int a, b; Objects.equals(a, b)` → `a == b` |  |
| P014 | Objects.hash mit einem Argument | 2* | ⚠️ | baseline:ObjectsHashCodeUnnecessaryVarargs | `Objects.hash(x)` → `Objects.hashCode(x)` | Hashwert ändert sich |
| P015 | Boxed Funktionsinterface statt primitiver Variante | 2 | ⚠️ | errorprone:LambdaFunctionalInterface | `private void f(Function<Integer, Integer> g)` → `IntUnaryOperator` | nur private/lokale Signaturen |
| P016 | BigInteger/BigDecimal-Konstante neu erzeugt ★ | 1 | ✅ | picnic:BigDecimalRules.BigDecimalZero+BigDecimalOne+BigDecimalTwo+BigDecimalTen, pmd:BigIntegerInstantiation | `new BigInteger("0")` → `BigInteger.ZERO; BigDecimal.valueOf(0)` → `BigDecimal.ZERO` |  |
| P017 | HashMap/HashSet mit Enum-Schlüssel statt EnumMap/EnumSet | 2* | ⚠️ | pmd:UseEnumCollections, intellij:MapReplaceableByEnumMap, netbeans:Map replaceable with EnumMap (performance), intellij:SetReplaceableByEnumSet, netbeans:Set replaceable with EnumSet (performance) | `new HashMap<Color, X>()` → `new EnumMap<>(Color.class)` | nur Enums der Datei oder JDK-Enums |
| P018 | toArray mit vorab dimensioniertem Array | 1 | ✅ | netbeans:Inefficient Collection.toArray (performance), intellij:ToArrayCallWithZeroLengthArrayArgument, pmd:OptimizableToArrayCall | `list.toArray(new String[list.size()])` → `list.toArray(new String[0])` | Option: Richtung wählbar (Länge 0 Standard, IntelliJ erlaubt auch vorab dimensioniert) |
| P019 | Leeres Array neu erzeugt statt geteilter Konstante | 2 | ✅ | intellij:UnnecessaryEmptyArrayUsage, intellij:ZeroLengthArrayInitialization | `new Item[0]` → `Item.EMPTY_ARRAY` | Fix nur, wenn Konstante in der Datei existiert; toArray(new T[0]) ausgenommen (P018) |
| P020 | Arrays.asList mit höchstens einem Element | 2* | ⚠️ | intellij:ArraysAsListWithZeroOrOneArgument | `Arrays.asList("a")` → `Collections.singletonList("a")` | set() danach nicht mehr möglich |
| P021 | new HashMap(int) als erwartete Größe missverstanden | 2* | ⚠️ | baseline:AvoidNewHashMapInt | `new HashMap<>(n)` → `HashMap.newHashMap(n)` | newHashMap ab 19 |
| P022 | List.containsAll mit quadratischer Laufzeit | 2* | ⚠️ | intellij:SlowListContainsAll | `list.containsAll(c)` → `new HashSet<>(list).containsAll(c)` |  |
| P023 | Set.removeAll mit List-Argument langsam | 2* | ⚠️ | intellij:SlowAbstractSetRemoveAll | `set.removeAll(list)` → `list.forEach(set::remove)` |  |
| P024 | list.remove(i) in Schleife statt subList().clear() | 2* | ✅ | intellij:ListRemoveInLoop | `for (…) list.remove(from);` → `list.subList(from, to).clear();` |  |
| P025 | Iteration über keySet() mit get() statt entrySet() | 2* | ⚠️ | intellij:KeySetIterationMayUseEntrySet | `for (K k : map.keySet()) use(map.get(k));` → `for (var e : map.entrySet())` |  |
| P026 | Kopie einer bereits unveränderlichen Collection | 2 | ✅ | errorprone:UnnecessaryCopy | `ImmutableList.copyOf(List.of(a))` → `List.of(a)` |  |
| P027 | ImmutableList nur für contains genutzt | 2 | ✅ | errorprone:ImmutableSetForContains | `private static final ImmutableList<A> L … L.contains(x)` → `ImmutableSet` |  |
| P028 | stream().count() statt size() | 2* | ✅ | intellij:ReplaceInefficientStreamCount | `list.stream().count()` → `list.size()` |  |
| P029 | Überflüssiges map()/boxed() vor count() | 2* | ⚠️ | intellij:MappingBeforeCount | `list.stream().map(f).count()` → `list.stream().count()` | f mit Seiteneffekt |
| P030 | Collector statt direkter Stream-Operation | 1 | ✅ | picnic:StreamRules.StreamMapToIntSum+StreamMapToDoubleSum+StreamMapToLongSum+StreamMapToIntSummaryStatistics+StreamMapToDoubleSummaryStatistics+StreamMapToLongSummaryStatistics+StreamCount+StreamReduce+StreamReduceWithObject+StreamFilterCollect+StreamMapCollect+StreamFlatMapCollect | `s.collect(counting())` → `s.count(); s.collect(summingInt(f))` → `s.mapToInt(f).sum()` |  |
| P031 | sorted().findFirst() statt min() | 1 | ⚠️ | baseline:SortedStreamFirstElement | `s.sorted().findFirst()` → `s.min(naturalOrder())` |  |
| P032 | flatMap(Optional::stream) statt mapMulti | 1 | ✅ | baseline:StreamFlatMapOptional | `s.flatMap(Optional::stream)` → `s.mapMulti(Optional::ifPresent)` | mapMulti ab 16 |
| P033 | Teures Argument eager ausgewertet statt Supplier-Variante | 1 | ⚠️ | intellij:ExplicitArgumentCanBeLambda, baseline:OptionalOrElseMethodInvocation, picnic:OptionalOrElseGet | `opt.orElse(load())` → `opt.orElseGet(() -> load())` |  |
| P034 | String.format als Precondition-Meldung | 2 | ✅ | errorprone:ExpensiveLenientFormatString, baseline:PreconditionsConstantMessage | `checkArgument(c, String.format("%s", x))` → `checkArgument(c, "%s", x)` |  |
| P035 | Schleife ohne break nach gefundenem Ergebnis | 2 | ✅ | intellij:IfStatementMissingBreakInLoop | `for (…) { if (a[i] == x) found = true; }` → `… { found = true; break; }` |  |
| P036 | Schleife über toCharArray statt charAt | 2* | ⚠️ | errorprone:LoopOverCharArray | `for (char c : s.toCharArray())` → `for (int i …) s.charAt(i)` |  |
| P037 | Endrekursion statt Schleife | 1 | – | intellij:TailRecursion | `int f(int n) { … return f(n - 1); }` |  |
| P038 | Objekterzeugung in equals/hashCode/compareTo | 1 | – | intellij:ObjectInstantiationInEqualsHashCode | `public int hashCode() { return Objects.hash(a, b); }` |  |
| P039 | Lokale Atomic-Variable ohne Nebenläufigkeit | 2 | ⚠️ | errorprone:UnnecessaryAsync | `AtomicInteger n = new AtomicInteger();` → `int n = 0;` | nicht bei Lambda-Capture |
| P040 | Calendar nur für aktuelles Datum erzeugt | 2 | ✅ | pmd:AvoidCalendarDateCreation | `Calendar.getInstance().getTime()` → `new Date()` |  |
| P041 | Expliziter System.gc()-Aufruf | 1 | – | pmd:DoNotCallGarbageCollectionExplicitly, intellij:SystemGC | `System.gc();` |  |
| P042 | FileInputStream/FileOutputStream statt Files.newInputStream | 2 | ⚠️ | pmd:AvoidFileStream, intellij:IOStreamConstructor, picnic:FileRules.FilesNewInputStreamPathOf+FilesNewInputStreamFileToPath+FilesNewOutputStreamPathOf+FilesNewOutputStreamFileToPath+FilesNewBufferedReader+FilesNewBufferedReaderWithCharset | `new FileInputStream(f)` → `Files.newInputStream(f.toPath())` | andere Exception-Typen |
| P043 | Mehrere File-Attribute einzeln gelesen | 2* | ⚠️ | intellij:BulkFileAttributesRead | `f.isDirectory(); f.length(); f.lastModified();` → `Files.readAttributes(…)` |  |
| P044 | InputStream/FilterOutputStream ohne Bulk-read/write | 2 | – | baseline:FilterOutputStreamSlowMultibyteWrite, errorprone:InputStreamSlowMultibyteRead | `class S extends InputStream { int read() {…} }` |  |
| P045 | Scanner.useDelimiter zum Lesen des ganzen Streams | 2 | – | errorprone:ScannerUseDelimiter | `new Scanner(in).useDelimiter("\\A").next()` |  |
| P046 | ICC_Profile.getInstance(String) durchsucht den Classpath | 2 | – | errorprone:ICCProfileGetInstance | `ICC_Profile.getInstance("x.icc")` |  |
| P047 | getHostName() löst Reverse-DNS aus | 2* | – | baseline:ReverseDnsLookup | `InetAddress a; a.getHostName()` |  |
| P048 | new InetSocketAddress(host, port) löst DNS sofort auf | 2 | ⚠️ | baseline:DnsLookup | `new InetSocketAddress(host, 80)` → `InetSocketAddress.createUnresolved(host, 80)` |  |
| P049 | StringBuffer statt StringBuilder | 2 | ⚠️ | modernizer:StringBuffer → StringBuilder, netbeans:Useless use of StringBuffer (performance), intellij:StringBufferReplaceableByStringBuilder | `StringBuffer sb = new StringBuffer();` → `StringBuilder sb = new StringBuilder();` | ab 5; Fix safe bei lokaler, nicht entkommender Variable |

### A: Design und Aufbau

| Code | Regel | Stufe | Fix | Herkunft | Beispiel | Hinweis |
|---|---|---|---|---|---|---|
| A018 | Abstrakte Klasse ohne Zustand kann Interface sein | 1 | ⚠️ | intellij:ClassMayBeInterface, netbeans:Class may be interface (class_structure) | `abstract class A { abstract void run(); }` → `interface A` |  |
| A019 | Leere Methode in abstrakter Klasse statt abstract | 1 | ⚠️ | intellij:NoopMethodInAbstractClass, pmd:EmptyMethodInAbstractClassShouldBeAbstract | `abstract class A { void hook() {} }` → `abstract void hook();` |  |
| A020 | public-Konstruktor in abstrakter Klasse | 1 | ⚠️ | intellij:NonProtectedConstructorInAbstractClass | `abstract class A { public A() {} }` → `protected A()` |  |
| A021 | Funktionales Interface ohne @FunctionalInterface | 2* | ✅ | intellij:InterfaceMayBeAnnotatedFunctional, pmd:ImplicitFunctionalInterface | `interface Action { void run(); }` → `@FunctionalInterface interface Action` | nur Interfaces ohne Obertypen außerhalb der Datei |
| A022 | Leeres Marker-Interface | 1 | – | intellij:MarkerInterface | `interface Tagged {}` | A005 erlaubt Marker-Interfaces |
| A023 | Konstante in Interface deklariert | 1 | – | intellij:ConstantDeclaredInInterface | `interface Config { int MAX = 10; }` | weiter als A005 (nur Konstanten-Interfaces) |
| A024 | Annotation-Implementierung ist veränderlich | 2 | – | errorprone:ImmutableAnnotationChecker | `class A implements MyAnno { int x; }` |  |
| A025 | Leere Klasse oder leere Datei | 1 | – | intellij:EmptyClass | `class Foo {}` |  |
| A026 | package-info.java ohne package-Anweisung | 1 | – | intellij:PackageInfoWithoutPackage | `package-info.java nur mit Javadoc, ohne package` | A007 prüft package-info.java ohne Typen nicht |
| A027 | Typdeklaration in package-info.java | 1 | – | errorprone:PackageInfo | `package-info.java mit class A {}` |  |
| A028 | Lokale Klasse in einem Block | 1 | – | intellij:LimitedScopeInnerClass | `void f() { class Helper {} }` |  |
| A029 | Anonyme Klasse kann statische verschachtelte Klasse sein | 2 | ⚠️ | intellij:AnonymousInnerClassMayBeStatic | `new Runnable() { public void run() { System.out.println(); } }` | Ergänzung zu A012 |
| A030 | Instanz-Initialisierungsblock | 1 | – | intellij:ClassInitializer, pmd:NonStaticInitializer | `class A { { init(); } }` | B025 deckt nur Double-Brace ab |
| A031 | Instanz-Initialisierer ohne Instanzbezug kann static sein | 2 | ⚠️ | intellij:ClassInitializerMayBeStatic, netbeans:Initializer may be static (class_structure) | `{ LOG.info("x"); }` → `static { … }` |  |
| A032 | Klasse referenziert eigene Unterklasse | 2 | – | intellij:ClassReferencesSubclass | `class Entity { SimpleEntity x; } class SimpleEntity extends Entity` |  |
| A033 | Klasse erweitert konkrete Collection | 2 | – | intellij:ExtendsConcreteCollection | `class Names extends ArrayList<String>` |  |
| A034 | Klasse implementiert Comparable und Comparator | 2 | – | errorprone:ComparableAndComparator | `class A implements Comparable<A>, Comparator<A>` |  |
| A035 | Klasse implementiert Iterable und Iterator | 2 | – | errorprone:IterableAndIterator | `class A implements Iterable<T>, Iterator<T>` |  |
| A036 | compareTo ohne implements Comparable | 1 | – | errorprone:MissingImplementsComparable | `class A { public int compareTo(A o) }` |  |
| A037 | Klasse erweitert Utility-Klasse | 2* | – | intellij:ExtendsUtilityClass | `class MyUtils extends StringUtils (StringUtils in der Datei)` |  |
| A038 | Utility-Klasse mit new instanziiert | 2 | – | intellij:InstantiationOfUtilityClass | `new MathUtils() (alle Member static, in der Datei)` |  |
| A039 | Klasse ohne zugänglichen Konstruktor und ohne statische Methode | 1 | – | pmd:MissingStaticMethodInNonInstantiatableClass | `class A { private A() {} void f() {} }` |  |
| A040 | Singleton mit überladenem getInstance | 1 | – | pmd:SingleMethodSingleton | `static A getInstance() / static A getInstance(int x)` |  |
| A041 | Builder mit Fabrikmethode hat nicht-privaten Konstruktor | 1 | ✅ | baseline:ImplicitPublicBuilderConstructor | `static Builder builder() {…} static class Builder { }` → `private Builder() {}` |  |
| A042 | Builder-Methode gibt nicht this zurück | 2 | – | errorprone:BuilderReturnThis | `Builder setX(int x) { return new Builder(); }` |  |
| A043 | public-Member in nicht-öffentlichem Typ | 1 | ✅ | pmd:PublicMemberInNonPublicType, intellij:PublicConstructorInNonPublicClass, netbeans:Public constructor in non-public class (class_structure), errorprone:EffectivelyPrivate | `class A { public void f() {} }` | nicht für @Override/Interface-Implementierungen |
| A044 | protected-Member in finaler Klasse ★ | 1 | ✅ | pmd:ProtectedMemberInFinalClass, pmd:AvoidProtectedFieldInFinalClass, pmd:AvoidProtectedMethodInFinalClassNotExtending, netbeans:Protected member in final class (class_structure), errorprone:ProtectedMembersInFinalClass, intellij:ProtectedMemberInFinalClass | `final class A { protected int x; }` | Methoden nur ohne Oberklasse bzw. ohne @Override |
| A045 | Weniger sichtbarer Typ in sichtbarerer Signatur | 2 | – | errorprone:ExposedPrivateType, intellij:ClassEscapesItsScope | `private static class P {} public P f()` |  |
| A046 | @VisibleForTesting-Member nicht package-private | 1 | ⚠️ | baseline:VisibleForTestingPackagePrivate | `@VisibleForTesting public void f()` → `void f()` |  |
| A047 | Private Methode nur von innerer Klasse genutzt | 2 | – | intellij:MethodOnlyUsedFromInnerClass | `private void helper() nur in class Inner aufgerufen` |  |
| A048 | Statisches Feld nicht final | 1 | ⚠️ | errorprone:NonFinalStaticField, intellij:StaticNonFinalField | `static int count;` → `static final` | nicht-private Felder meldet schon A003; diese Regel auch private |
| A049 | Finales Feld mit Konstante kann static sein | 1 | ✅ | errorprone:FieldCanBeStatic, intellij:FieldMayBeStatic, pmd:FinalFieldCouldBeStatic | `private final int MAX = 5;` → `private static final` |  |
| A050 | Feld nur in einer Methode benutzt, kann lokal sein | 2 | ⚠️ | errorprone:FieldCanBeLocal, intellij:FieldCanBeLocal, pmd:SingularField | `private int tmp; nur in einer Methode benutzt` |  |
| A051 | Statisches Collection-Feld | 2 | – | intellij:StaticCollection | `static List<String> cache = new ArrayList<>();` |  |
| A052 | Nie veränderte Member-Collection nicht unveränderlich deklariert | 2 | ⚠️ | errorprone:ImmutableMemberCollection | `private final List<A> l = ImmutableList.of();` → `ImmutableList<A>` |  |
| A053 | Feld fehlt in toString() | 2 | – | intellij:FieldNotUsedInToString | `String toString() { return "A[" + a + "]"; } ohne Feld b` |  |
| A054 | Feld mit Setter, aber ohne Getter | 2 | – | intellij:FieldHasSetterButNoGetter | `private int x; void setX(int x) { … } ohne getX()` |  |
| A055 | Enum mit veränderlichem Zustand | 2* | – | intellij:NonFinalFieldInEnum, errorprone:ImmutableEnumChecker | `enum E { A; int count; }` | nicht-finale Felder Stufe 1; veränderliche Feldtypen nur bei lokal bekanntem Typ |
| A056 | Zeitwert als primitiver Typ statt Duration/Instant | 2 | – | errorprone:StronglyTypeTime | `private static final long TIMEOUT_MS = 100; nur in Duration.ofMillis(TIMEOUT_MS)` |  |
| A057 | Veränderliches Feld (Array/Collection/Date) ungeschützt zurückgegeben | 2 | – | pmd:MethodReturnsInternalArray, netbeans:Return of Array Field (encapsulation), netbeans:Return of Collection Field (encapsulation), netbeans:Return of Date or Calendar Field (encapsulation), intellij:AssignmentOrReturnOfFieldWithMutableType | `return this.values;` → `return values.clone();` | intellij deckt auch die Zuweisung ab (Regel darunter) |
| A058 | Veränderlicher Parameter (Array/Collection/Date) ungeschützt gespeichert | 2 | – | pmd:ArrayIsStoredDirectly, netbeans:Assignment to Array Field from Parameter (encapsulation), netbeans:Assignment to Collection Field from Parameter (encapsulation), netbeans:Assignment to Date or Calendar Field from Parameter (encapsulation) | `this.values = values;` → `this.values = values.clone();` |  |
| A059 | public static final Array ist veränderlich ★ | 1 | – | errorprone:MutablePublicArray, intellij:PublicStaticArrayField, intellij:PublicStaticCollectionField | `public static final int[] X = {1};` |  |
| A060 | Methode ohne Instanzbezug kann static sein | 2 | ✅ | errorprone:MethodCanBeStatic, intellij:MethodMayBeStatic, eclipse:COMPILER_PB_MISSING_STATIC_ON_METHOD | `private int f(int x) { return x; }` → `private static` | Fix nur für private/finale Methoden sicher |
| A061 | static-Methode als final markiert | 1 | ✅ | intellij:FinalStaticMethod | `static final void x()` → `static void x()` |  |
| A062 | Fehlende @Override-Annotation | 2* | ✅ | errorprone:MissingOverride, pmd:MissingOverride, eclipse:COMPILER_PB_MISSING_OVERRIDE_ANNOTATION, intellij:MissingOverrideAnnotation, netbeans:Add @Override Annotation (rules15) | `public String toString()` → `@Override public String toString()` | nur equals/hashCode/toString u. ä. und Obertypen der Datei |
| A063 | Überschreibbare, nicht leere Methode (nicht final/abstract) | 1 | – | intellij:DesignForExtension | `public void process() { … } in nicht-finaler Klasse` | wie CS DesignForExtension |
| A064 | Leere Methode kann vermutlich entfallen | 2* | ⚠️ | picnic:EmptyMethod | `private void init() {}` |  |
| A065 | Methode gibt stets dieselbe Konstante zurück | 2* | – | intellij:SameReturnValue | `private int f() { if (a) return 1; return 1; }` | nur private/final/static Methoden |
| A066 | Rückgabewert einer privaten Methode nie verwendet | 2* | – | intellij:UnusedReturnValue | `private int f() {…} — nur als f(); aufgerufen` |  |
| A067 | Parameter einer privaten Methode erhält immer denselben Wert | 2* | – | intellij:SameParameterValue | `private void p(int x, int y) — stets p(0, …)` |  |
| A068 | return null | 1 | – | intellij:ReturnNull | `return null;` |  |
| A069 | Methode liefert veränderliche und unveränderliche Collections | 2 | – | errorprone:MixedMutabilityReturnType | `return c ? new ArrayList<>() : List.of();` |  |
| A070 | Rückgabe einer Instanz einer inneren/anonymen Klasse | 2 | – | intellij:ReturnOfInnerClass | `return new Runnable() { … };` |  |
| A071 | Letzter Array-Parameter kann varargs sein | 1 | ⚠️ | pmd:UseVarargs, intellij:MethodCanBeVariableArityMethod | `void f(String[] args)` → `void f(String... args)` |  |
| A072 | Varargs-Methode ist überladen | 1 | – | intellij:OverloadedVarargsMethod | `void f(String s); void f(String... s);` |  |
| A073 | Generische Varargs-Methode ohne @SafeVarargs | 2 | ⚠️ | intellij:SafeVarargsDetector | `static <T> List<T> of(T... xs)` → `@SafeVarargs static <T> …` |  |
| A074 | Überladungen mit kollidierenden funktionalen Interfaces | 2* | – | intellij:LambdaUnfriendlyMethodOverload, errorprone:FunctionalInterfaceClash | `void f(Supplier<T> s); void f(Callable<T> c);` |  |
| A075 | Parameterreihenfolge der Überladungen inkonsistent | 1 | – | errorprone:InconsistentOverloads | `f(int a, String b); f(String b, int a, int c)` |  |
| A076 | Überladungen mit gleicher Parameteranzahl | 1 | – | intellij:OverloadedMethodsWithSameNumberOfParameters | `void f(int a); void f(String s);` |  |
| A077 | boolean-Parameter in öffentlicher Methode | 1 | – | intellij:BooleanParameter | `public void show(boolean animate)` |  |
| A078 | Boolean-Literal als Argument ohne Parameterkommentar | 2* | ✅ | errorprone:BooleanParameter | `f(true)` → `f(/* enabled= */ true)` | nur Methoden der Datei (Parametername nötig) |
| A079 | Implementierungstyp statt Interface in Deklaration/Signatur | 2 | ⚠️ | pmd:LooseCoupling, intellij:DeclareCollectionAsInterface, errorprone:NonApiType, errorprone:PreferredInterfaceType | `ArrayList<String> list = new ArrayList<>();` → `List<String> list` | Option Gegenrichtung (EP PreferredInterfaceType: spezifischer Typ, z. B. ImmutableList, nur 2*) |
| A080 | Objekt-Array in öffentlicher Signatur | 1 | – | errorprone:AvoidObjectArrays | `public String[] names()` → `List<String>` |  |
| A081 | Parameter vom Typ Iterable<Path> | 2 | – | errorprone:IterablePathParameter | `void f(Iterable<Path> p)` → `Collection<Path>` |  |
| A082 | Optional als Feld- oder Parametertyp | 2 | – | intellij:OptionalUsedAsFieldOrParameterType | `void f(Optional<String> name)` |  |
| A083 | Optional mit Collection- oder Array-Typargument | 2 | – | intellij:OptionalContainsCollection | `Optional<List<String>> find()` |  |
| A084 | Optional in Optional | 1 | – | picnic:NestedOptionals | `Optional<Optional<String>> x;` |  |
| A085 | Verwendung von StringTokenizer | 2 | – | intellij:StringTokenizer | `new StringTokenizer(s, ",")` |  |
| A086 | Typparameter mit finaler Schranke | 2* | ⚠️ | intellij:TypeParameterExtendsFinalClass, eclipse:COMPILER_PB_FINAL_PARAMETER_BOUND | `<T extends String> void f(T t)` → `void f(String t)` | nur final-Klassen der Datei bzw. bekannte JDK-Typen |
| A087 | Typparameter nur im Rückgabetyp | 1 | – | errorprone:TypeParameterUnusedInFormals | `<T> T get(String key)` |  |
| A088 | equals mit getClass statt instanceof | 1 | ⚠️ | errorprone:EqualsGetClass | `getClass() != o.getClass()` → `!(o instanceof Foo)` |  |
| A089 | instanceof/getClass() auf this | 1 | – | intellij:InstanceofThis | `if (this instanceof Sub)` |  |
| A090 | clone() nicht public | 1 | ✅ | pmd:CloneMethodMustBePublic, intellij:NonPublicClone | `protected Object clone()` → `public Foo clone()` |  |
| A091 | clone() liefert nicht den eigenen Klassentyp | 1 | ⚠️ | pmd:CloneMethodReturnTypeMustMatchClassName, intellij:CloneReturnsClassType | `public Object clone()` → `public Foo clone()` |  |
| A092 | clone() deklariert CloneNotSupportedException nicht | 1 | ⚠️ | intellij:CloneDeclaresCloneNotSupported | `protected Object clone()` → `… throws CloneNotSupportedException` |  |
| A093 | clone() nicht final | 1 | ⚠️ | intellij:NonFinalClone | `public Foo clone()` → `public final Foo clone()` |  |
| A094 | readResolve/writeReplace nicht protected | 2* | ✅ | intellij:ReadResolveAndWriteReplaceProtected | `private Object readResolve()` → `protected Object readResolve()` |  |
| A095 | Throwable überschreibt equals/hashCode | 2 | – | errorprone:ThrowableEqualsHashCode | `class E extends Exception { boolean equals(Object o) }` |  |
| A096 | Exception überschreibt toString statt getMessage | 2 | ⚠️ | errorprone:OverrideThrowableToString | `class E extends Exception { public String toString() }` |  |
| A097 | Label, break/continue mit Label | 1 | – | intellij:LabeledStatement, pmd:LabeledStatement, intellij:BreakStatementWithLabel, intellij:ContinueStatementWithLabel | `outer: for (…) { … continue outer; }` | Option: nur Labels oder auch break/continue |
| A098 | Zuweisung von null außerhalb der Deklaration | 1 | – | intellij:AssignmentToNull | `cache = null;` |  |
| A099 | Variable in zu weitem Gültigkeitsbereich deklariert | 2 | ⚠️ | intellij:TooBroadScope | `int x = 0; if (c) { use(x); }` → `in den if-Block verschieben` | verwandt mit A014 (Abstand statt Block) |
| A100 | Zeichenliteral ohne benannte Konstante | 1 | – | intellij:MagicCharacter | `if (c == ';')` → `if (c == SEPARATOR)` | Ergänzung zu A015 |
| A101 | Auskommentierter Code | 1 | – | intellij:CommentedOutCode | `// int x = compute();` |  |
| A102 | Veraltetes Element wird in der Datei noch verwendet | 2 | – | intellij:DeprecatedIsStillUsed | `@Deprecated void old() {} void x() { old(); }` |  |
| A103 | native-Methode deklariert | 1 | – | intellij:NativeMethods | `native void read();` |  |
| A104 | JDBC-Verbindung über DriverManager statt DataSource | 2 | – | intellij:DriverManagerGetConnection | `DriverManager.getConnection(url)` |  |
| A105 | Proxy mit nicht-konstanten Interface-Typen (native-image) | 2 | – | baseline:ProxyNonConstantType | `Proxy.newProxyInstance(cl, new Class<?>[] {type}, h)` |  |
| A106 | Absolute AWT/Swing-Ausrichtung statt relativer | 2 | ⚠️ | intellij:AbsoluteAlignmentInUserInterface | `panel.add(label, BorderLayout.NORTH)` → `BorderLayout.PAGE_START` |  |
| A107 | getInstance() liefert jedes Mal neue Instanz | 1 | – | pmd:SingletonClassReturningNewInstance | `static A getInstance() { return new A(); }` |  |
| A108 | Aufruf von Enum.ordinal() | 1 | – | errorprone:EnumOrdinal | `color.ordinal()` |  |

### N: Namen

| Code | Regel | Stufe | Fix | Herkunft | Beispiel | Hinweis |
|---|---|---|---|---|---|---|
| N015 | Methode heißt wie ihre Klasse ★ | 1 | – | pmd:MethodWithSameNameAsEnclosingClass, eclipse:COMPILER_PB_METHOD_WITH_CONSTRUCTOR_NAME, intellij:MethodNameSameAsClassName | `class Foo { void Foo() {} }` | meist ein verunglückter Konstruktor |
| N016 | Methode heißt wie die Oberklasse | 1 | – | intellij:MethodNameSameAsParentName | `class B extends A { void A() {} }` |  |
| N017 | Methodennamen unterscheiden sich nur in Groß-/Kleinschreibung | 1 | – | intellij:MisspelledMethodName | `int hashcode() neben hashCode()` |  |
| N018 | Feld heißt wie eine Methode | 1 | – | pmd:AvoidFieldNameMatchingMethodName | `int size; int size() {…}` |  |
| N019 | Feld heißt wie die Klasse | 1 | – | pmd:AvoidFieldNameMatchingTypeName | `class Foo { int foo; }` |  |
| N020 | Variablenname gleich Typname | 1 | – | errorprone:VariableNameSameAsType | `Foo Foo;` |  |
| N021 | Feld und Parameter unterscheiden sich nur in Groß-/Kleinschreibung | 2 | ⚠️ | errorprone:InconsistentCapitalization | `int fooBar; void f(int foobar)` |  |
| N022 | Überschreibende Methode vertauscht Parameternamen | 2* | – | errorprone:OverridingMethodInconsistentArgumentNamesChecker | `f(int a, int b) überschrieben als f(int b, int a)` | nur Obertypen der Datei |
| N023 | Feld in CONSTANT_CASE ist nicht static final | 1 | – | errorprone:ConstantField | `private int MAX = 5;` |  |
| N024 | Klasse heißt wie ihre Oberklasse | 1 | – | intellij:ClassNameSameAsAncestorName | `class List extends java.util.List` |  |
| N025 | Klassenname beginnt mit Paketnamen | 1 | – | intellij:ClassNamePrefixedWithPackageName | `package util; class UtilHelper` |  |
| N026 | Klassenname sieht aus wie ein Typparameter | 1 | – | errorprone:ClassNamedLikeTypeParameter | `class T {}` |  |
| N027 | Exception-Suffix passt nicht zur Oberklasse | 2* | – | intellij:ExceptionNameDoesntEndWithException, intellij:NonExceptionNameEndsWithException | `class Fail extends Exception; class FooException {}` | beide Richtungen; Oberklasse muss lokal erkennbar sein |
| N028 | Typname überdeckt Klasse aus java.lang | 2 | – | errorprone:AvoidCommonTypeNames | `class String {}` |  |
| N029 | Gleicher einfacher Name für verschiedene Typen | 2 | – | errorprone:SameNameButDifferent | `innere Klasse Entry neben import Map.Entry` |  |
| N030 | Boolesche Methode beginnt nicht mit Fragewort | 1 | – | intellij:BooleanMethodNameMustStartWithQuestion, pmd:BooleanGetMethodName | `boolean getActive()` → `isActive()` | Präfixliste konfigurierbar (is, has, can …) |
| N031 | Name widerspricht Typ (isX ohne boolean, setX mit Rückgabe) | 1 | – | pmd:LinguisticNaming, intellij:NonBooleanMethodNameMayNotStartWithQuestion | `int isValid() / boolean hasFoo = 3` |  |
| N032 | Methode stream() liefert keinen Stream | 1 | – | errorprone:PublicApiNamedStreamShouldReturnStream | `public List<A> stream()` |  |
| N033 | Negativ benannter boolescher Name | 1 | – | errorprone:NegativeBoolean, intellij:NegativelyNamedBooleanVariable | `boolean isNotValid` |  |
| N034 | Name zu kurz oder zu lang | 1 | – | pmd:ShortClassName, pmd:ShortMethodName, pmd:ShortVariable, pmd:LongVariable | `int q; String aVeryLongVariableNameIndeed;` | Grenzen je Namensart konfigurierbar |
| N035 | Nichtssagender Name | 1 | – | intellij:QuestionableName | `int foo; String bar;` |  |
| N036 | Standardname (i, s, c …) mit unpassendem Typ | 1 | – | intellij:StandardVariableNames | `String i; / long b;` |  |
| N037 | Logger-Feld nicht einheitlich benannt | 2 | ⚠️ | baseline:ConsistentLoggerName | `private static final Logger LOGGER = …` → `log` | SLF4J, Log4j 2, JUL; Name konfigurierbar, |
| N038 | Nicht-ASCII-Zeichen außerhalb von Kommentaren und Literalen | 1 | – | errorprone:UnicodeInCode | `int größe;` |  |

### C: Komplexität und Größe

| Code | Regel | Stufe | Fix | Herkunft | Beispiel | Hinweis |
|---|---|---|---|---|---|---|
| C023 | Summe der Komplexität einer Klasse zu hoch | 1 | – | intellij:ClassComplexity | `Klasse mit Gesamtkomplexität > 80` |  |
| C024 | Anonyme Klasse zu komplex | 1 | – | intellij:AnonymousClassComplexity | `new Runnable() { /* Komplexität > 3 */ }` |  |
| C025 | Anonyme Klasse mit zu vielen Methoden | 1 | – | intellij:AnonymousClassMethodCount | `new Listener() { void a(){} void b(){} void c(){} }` |  |
| C026 | Zu viele Felder | 1 | – | pmd:TooManyFields, intellij:FieldCount | `(Anzahl Felder über Schwellwert)` |  |
| C027 | Zu viele Konstruktoren | 1 | – | intellij:ConstructorCount | `Klasse mit > 5 Konstruktoren` |  |
| C028 | Zu viele öffentliche Member | 1 | – | pmd:ExcessivePublicCount | `(Anzahl public-Member über Schwellwert)` |  |
| C029 | Zu viele Imports | 1 | – | pmd:ExcessiveImports | `(Anzahl Imports über Schwellwert)` |  |
| C030 | Methode referenziert zu viele Klassen | 1 | – | intellij:MethodCoupling | `Methode mit > 10 referenzierten Typen` | Methodenebene; Klassenebene C018 abgelehnt |
| C031 | Zu tief verschachtelte Anweisungen in einer Methode | 1 | – | intellij:NestingDepth | `Verschachtelungstiefe > 5` | alle Blockarten zusammen, anders als C014–C016 |
| C032 | Zu tief verschachtelte innere Klassen | 1 | – | intellij:ClassNestingDepth | `class A { class B { class C { } } }` |  |
| C033 | Verschachtelter Bedingungsausdruck | 1 | – | intellij:NestedConditionalExpression | `a == 10 ? b == 20 ? 10 : a : b` |  |
| C034 | Verschachteltes switch | 1 | – | intellij:NestedSwitchStatement | `switch (a) { case 1 -> switch (b) { … } }` |  |
| C035 | Sehr tief verschachtelter Ausdruck | 1 | – | errorprone:DeeplyNested | `a(b(c(d(…))))` |  |
| C036 | if-else-Kette mit zu vielen Zweigen | 1 | – | intellij:IfStatementWithTooManyBranches | `if … else if … (> 3 Zweige)` |  |
| C037 | switch mit zu vielen case-Labels | 1 | – | intellij:SwitchStatementWithTooManyBranches | `switch (x) { /* 30 cases */ }` |  |
| C038 | Zu viele Anweisungen pro case | 1 | – | pmd:SwitchDensity, intellij:SwitchStatementDensity | `switch (x) { case 1: a(); b(); c(); d(); … }` |  |
| C039 | Zu viele Negationen in einer Methode | 1 | – | intellij:ThreeNegationsPerMethod | `if (!a && b != c && !d)` |  |
| C040 | Arithmetischer Ausdruck mit zu vielen Termen | 1 | – | intellij:OverlyComplexArithmeticExpression | `a + b * c - d / e + f * g …` |  |
| C041 | Zu viele Steuervariablen in for-Schleife | 1 | – | pmd:ForLoopVariableCount | `for (int i = 0, j = 0; i < n; i++, j++)` |  |
| C042 | Zu großer Initialisierer für primitives Array | 1 | – | intellij:OverlyLargePrimitiveArrayInitializer | `int[] t = {1, 2, 3, /* … 5000 Werte */};` |  |

### I: Imports

| Code | Regel | Stufe | Fix | Herkunft | Beispiel | Hinweis |
|---|---|---|---|---|---|---|
| I006 | Import verschachtelter Typen mit allgemeinem Namen | 1 | ✅ | errorprone:BadImport | `import com.foo.Bar.Builder;` |  |
| I007 | Statischer Import eines Typs | 2 | ✅ | errorprone:UnnecessaryStaticImport | `import static java.util.Map.Entry;` → `import java.util.Map.Entry;` |  |
| I008 | Typ in der Datei unterschiedlich benannt | 2 | ✅ | errorprone:DifferentNameButSame | `Map.Entry und Entry gemischt` |  |
| I009 | Nicht-kanonische Variante einer Annotation importiert | 2 | ✅ | baseline:PreferCommonAnnotations | `import org.jetbrains.annotations.VisibleForTesting` → `com.google.common.annotations.VisibleForTesting` | Zuordnung konfigurierbar; verwandt mit I002 |

### D: Javadoc-Inhalt

| Code | Regel | Stufe | Fix | Herkunft | Beispiel | Hinweis |
|---|---|---|---|---|---|---|
| D001 | @exception statt @throws | 1 | ✅ | errorprone:PreferThrowsTag | `@exception IOException` → `@throws IOException` |  |
| D002 | {@link} im @throws-Tag | 1 | ✅ | errorprone:InvalidThrowsLink | `@throws {@link IOException}` → `@throws IOException` |  |
| D003 | Fehlerhaftes {@link} | 1 | ✅ | errorprone:InvalidLink | `{@link https://x}` → `<a href="…">` |  |
| D004 | Javadoc-Link auf das eigene Element | 1 | ✅ | intellij:UnnecessaryJavaDocLink | `/** @see #f() */ void f()` |  |
| D005 | URL im @see-Tag | 1 | ✅ | errorprone:UrlInSee | `@see https://x` → `@see <a href="https://x">x</a>` |  |
| D006 | URL als Klartext im Javadoc | 1 | ✅ | intellij:JavadocLinkAsPlainText | `https://x.org` → `<a href="https://x.org">https://x.org</a>` |  |
| D007 | Javadoc nur mit {@inheritDoc} | 1 | ✅ | intellij:UnnecessaryInheritDoc | `/** {@inheritDoc} */ @Override void f()` |  |
| D008 | {@inheritDoc} ohne überschriebene Methode | 2* | – | errorprone:InheritDoc | `/** {@inheritDoc} */ private int x;` |  |
| D009 | Javadoc-Text widerspricht der Deklaration | 1 | – | intellij:MismatchedJavadocCode | `/** @return true if … */ void f() / "returns null" bei primitivem Typ` |  |
| D010 | Ungültiges HTML im Javadoc | 1 | – | intellij:JavadocHtmlLint | `<p/> oder nicht geschlossenes <b>` |  |
| D011 | Unmaskierte HTML-Zeichen im Javadoc | 1 | ✅ | errorprone:UnescapedEntity | `/** List<String> */` → `{@code List<String>}` |  |
| D012 | HTML-Entität in {@code} | 1 | ✅ | errorprone:EscapedEntity | `{@code &lt;T&gt;}` → `{@code <T>}` |  |
| D013 | Ungültiges {@snippet} | 1 | – | errorprone:InvalidSnippet | `{@snippet :}` |  |
| D014 | Klassisches Javadoc als Markdown-Kommentar (///) | 1 | ✅ | intellij:MarkdownDocumentationCommentsMigration, errorprone:TraditionalJavadocToMarkdown | `/** Text {@link Foo} */` → `/// Text [Foo]` | ab 23 |

### X: Benutzerdefiniert

| Code | Regel | Stufe | Fix | Herkunft | Beispiel | Hinweis |
|---|---|---|---|---|---|---|
| X001 | Verbotene Methodenaufrufe (konfigurierbar) | 1 | – | intellij:SystemGetenv, intellij:SystemProperties | `System.getenv("HOME"); System.getProperty("x")` | Liste konfigurierbar; ergänzt IllegalType/IllegalInstantiation |
| X002 | Konfigurierter Typ als Hash-Schlüssel | 2* | – | errorprone:DiscouragedHashKey | `Set<Foo> s = new HashSet<>();` |  |
| X003 | @SuppressWarnings verboten oder nur mit Kommentar | 1 | – | intellij:SuppressionAnnotation, errorprone:SuppressWarningsWithoutExplanation | `@SuppressWarnings("unchecked") void f()` | Option: ganz verbieten (intellij) oder Begründungskommentar verlangen (EP); EP-Quelle war als A eingestuft |
| X004 | assert-Anweisung verwendet | 1 | – | baseline:BadAssert | `assert x > 0;` | Gegenrichtung zu assert-Regeln |

### T: Tests (neu)

| Code | Regel | Stufe | Fix | Herkunft | Beispiel | Hinweis |
|---|---|---|---|---|---|---|
| T001 | Testmethode ohne @Test (JUnit-3-Stil) wird nicht ausgeführt ★ | 2 | ⚠️ | errorprone:JUnit4TestNotRun, intellij:JUnit3StyleTestMethodInJUnit4Class, pmd:DetachedTestCase, pmd:UnitTestShouldUseTestAnnotation | `public void testFoo() {…}` → `@Test public void testFoo()` | JUnit 4/5; nur in Klassen mit anderen @Test-Methoden |
| T002 | setUp() ohne @Before/@BeforeEach | 2 | ✅ | errorprone:JUnit4SetUpNotRun, pmd:UnitTestShouldUseBeforeAnnotation | `public void setUp()` → `@BeforeEach void setUp()` | JUnit 4/5 |
| T003 | tearDown() ohne @After/@AfterEach | 2 | ✅ | errorprone:JUnit4TearDownNotRun, pmd:UnitTestShouldUseAfterAnnotation | `public void tearDown()` → `@AfterEach void tearDown()` | JUnit 4/5 |
| T004 | @BeforeClass/@AfterClass-Methode nicht static ★ | 1 | ✅ | errorprone:JUnit4ClassAnnotationNonStatic | `@BeforeClass public void init()` → `public static void init()` | JUnit 4 |
| T005 | Fehlerhafte Testdeklaration (private, static, Parameter, Rückgabe) ★ | 2 | ⚠️ | intellij:JUnitMalformedDeclaration, pmd:JUnitJupiterTestNoPrivateModifier | `@Test private void t()` → `@Test void t()` | JUnit 4/5 |
| T006 | Testmethode/-klasse unnötig public | 2 | ✅ | picnic:JUnitMethodDeclaration, pmd:JUnitJupiterTestShouldBePackagePrivate | `@Test public void t()` → `@Test void t()` | JUnit 5; picnic entfernt zusätzlich test-Präfix (→ T007) |
| T007 | test-Präfix bei @Test-Methode | 1 | ✅ | errorprone:UnnecessaryTestMethodPrefix | `@Test void testFoo()` → `@Test void foo()` | JUnit 4/5; Umbenennung, Aufrufer in der Datei mitziehen |
| T008 | Testklasse nicht package-private und final | 1 | ✅ | picnic:JUnitClassModifiers | `public class FooTest` → `final class FooTest` | JUnit 5 |
| T009 | JUnit-Versionen gemischt (Annotationen, @Rule, TestCase) ★ | 2 | ⚠️ | intellij:JUnitMixedFramework, pmd:WrongTestAnnotation, baseline:JUnit5RuleUsage, errorprone:JUnitAmbiguousTestClass | `import org.junit.Test; … @org.junit.jupiter.api.BeforeEach` | JUnit 3/4/5; per Imports erkennbar |
| T010 | JUnit-4-Assertions in JUnit-5-Test | 2 | ✅ | intellij:JUnit5AssertionsConverter | `Assert.assertEquals("m", a, b)` → `Assertions.assertEquals(a, b, "m")` | JUnit 4 → 5 |
| T011 | Veraltetes junit.framework.Assert | 2 | ✅ | intellij:UseOfObsoleteAssert | `junit.framework.Assert.assertEquals(a, b)` → `org.junit.Assert.assertEquals(a, b)` | JUnit 3 → 4 |
| T012 | Testklasse erbt von Assert statt statischem Import | 2 | ✅ | errorprone:ExtendingJUnitAssert | `class FooTest extends Assert` → `import static org.junit.Assert.*` | JUnit 4 |
| T013 | JUnit-4-Test nach JUnit 5 migrierbar | 2 | ⚠️ | intellij:JUnit5Converter | `@org.junit.Test` → `@org.junit.jupiter.api.Test` | JUnit 4 → 5; Migrationsregel, nicht im Standard |
| T014 | Leere @Before/@After-Methode | 1 | ✅ | errorprone:JUnit4EmptyMethods | `@Before public void setUp() {}` → `entfernen` | JUnit 4/5 |
| T015 | Testmethode direkt aufgerufen | 2 | – | errorprone:JUnitMethodInvoked | `@Test void a() {} … a();` | JUnit 4/5 |
| T016 | @Parameters ohne Parameterized-Runner | 2 | – | errorprone:ParametersButNotParameterized | `@Parameters static List<Object[]> data() ohne @RunWith(Parameterized.class)` | JUnit 4 |
| T017 | Parameterized-Test ohne gültige @Parameters-Methode | 2 | – | intellij:ParameterizedParametersStaticCollection | `@RunWith(Parameterized.class) mit nicht-statischer @Parameters-Methode` | JUnit 4 |
| T018 | @Theory ohne Theories-Runner | 2 | – | errorprone:TheoryButNoTheories | `@Theory void t(int x) ohne @RunWith(Theories.class)` | JUnit 4 |
| T019 | @Test in Klasse mit Enclosed-Runner wird nicht ausgeführt | 2 | – | errorprone:JUnit4TestsNotRunWithinEnclosed | `@RunWith(Enclosed.class) class T { @Test void a() }` | JUnit 4 |
| T020 | TestRule-Feld ohne @Rule | 2* | ✅ | errorprone:RuleNotRun | `public TemporaryFolder tmp = new TemporaryFolder();` → `@Rule public …` | JUnit 4; nur bei lokal bekanntem Regeltyp |
| T021 | @Test(expected)/ExpectedException statt assertThrows | 2 | ⚠️ | errorprone:TestExceptionChecker, errorprone:ExpectedExceptionChecker | `@Test(expected = E.class) void t() { f(); }` → `assertThrows(E.class, () -> f())` | JUnit 4 (ab 4.13) / 5 |
| T022 | try/fail/catch statt assertThrows | 2 | ⚠️ | errorprone:TryFailRefactoring, pmd:JUnitUseExpected | `try { f(); fail(); } catch (E e) {}` → `assertThrows(E.class, () -> f())` | JUnit 4/5, TestNG |
| T023 | Erwartete Exception ohne fail() im try | 2 | ⚠️ | errorprone:MissingFail | `try { f(); } catch (E e) { assertX(e); }` → `fail() nach f()` | JUnit 4/5 |
| T024 | Exception fangen und fail() aufrufen | 2 | ⚠️ | errorprone:CatchFail | `try { f(); } catch (E e) { fail(); }` → `f(); (throws E)` | JUnit 4/5 |
| T025 | Assertion-Fehler durch umgebendes catch verschluckt ★ | 2 | – | errorprone:AssertionFailureIgnored, intellij:MaskedAssertion, errorprone:TryFailThrowable | `try { f(); fail(); } catch (Throwable t) {}` | JUnit 4/5; catch von AssertionError/Error/Throwable |
| T026 | assertThrows-Lambda als Block mit einer Anweisung | 2 | ✅ | errorprone:AssertThrowsBlockToExpression | `assertThrows(E.class, () -> { f(); })` → `() -> f()` | JUnit 5; Spezialfall von S021 |
| T027 | assertThrows-Lambda mit mehreren Anweisungen | 2 | – | errorprone:AssertThrowsMultipleStatements | `assertThrows(E.class, () -> { a(); b(); })` | JUnit 5 |
| T028 | @MethodSource mit einfachen Literalen statt @ValueSource | 1 | ✅ | picnic:JUnitValueSource | `static Stream<Integer> args() { return Stream.of(1, 2); }` → `@ValueSource(ints = {1, 2})` | JUnit 5 |
| T029 | @ParameterizedTest ohne Parameter | 1 | ✅ | picnic:JUnitNullaryParameterizedTestDeclaration | `@ParameterizedTest @ValueSource(ints = 1) void t()` → `@Test void t()` | JUnit 5 |
| T030 | Arguments.of statt statisch importiertem arguments() | 2 | ✅ | picnic:JUnitRules.ArgumentsEnumeration | `Arguments.of(1, "a")` → `arguments(1, "a")` | JUnit 5; Stilregel |
| T031 | JUnit-Meta-Annotation ohne RUNTIME-Retention | 2 | ✅ | intellij:MetaAnnotationWithoutRuntimeRetention | `@Retention(SOURCE) @Test @interface MyTest {}` → `RUNTIME` | JUnit 5 |
| T032 | assertEquals mit vertauschtem expected/actual ★ | 1 | ✅ | errorprone:AssertEqualsArgumentOrderChecker, pmd:AssertEqualsArgumentOrder, intellij:MisorderedAssertEqualsArguments, netbeans:DN_assertEqualsMismatchedConstantVSReal (testing) | `assertEquals(result, 42)` → `assertEquals(42, result)` | JUnit 4/5, TestNG (umgekehrte Reihenfolge); Stufe 1 für Literal/Konstante als actual |
| T033 | assertEquals auf Arrays statt assertArrayEquals ★ | 2* | ⚠️ | intellij:AssertEqualsCalledOnArray, netbeans:DN_assertEqualsForArrays (testing) | `int[] a; assertEquals(new int[] {1}, a)` → `assertArrayEquals(new int[] {1}, a)` | JUnit 4/5; nur bei lokal bekanntem Array-Typ |
| T034 | assertEquals mit unvereinbaren Typen | 2* | – | intellij:AssertBetweenInconvertibleTypes | `assertEquals("1", 1)` | JUnit 4/5; nur Literale/lokal bekannte Typen |
| T035 | Assertion vergleicht Wert mit sich selbst ★ | 2 | – | errorprone:JUnitAssertSameCheck, errorprone:SelfAssertion | `assertSame(a, a); assertThat(x).isEqualTo(x)` | JUnit 4/5, AssertJ |
| T036 | Assertion mit konstantem Argument (assertTrue(true)) | 2 | – | pmd:UnnecessaryBooleanAssertion, intellij:ConstantAssertArgument | `assertTrue(true); assertFalse(false)` | JUnit 4/5 |
| T037 | Vereinfachbare Assertion ★ | 2 | ✅ | intellij:SimplifiableAssertion, pmd:SimplifiableTestAssertion | `assertEquals(true, x())` → `assertTrue(x()); assertTrue(a.equals(b))` → `assertEquals(a, b)` | JUnit 4/5, TestNG |
| T038 | Doppelte Assertion | 1 | ✅ | errorprone:DuplicateAssertion | `assertThat(x).isTrue(); assertThat(x).isTrue();` → `einmal` | JUnit 4/5, AssertJ |
| T039 | Gleitkomma-Toleranz kleiner als Rechengenauigkeit | 2 | – | errorprone:FloatingPointAssertionWithinEpsilon | `assertEquals(1.0, x, 1e-20)` | JUnit 4/5 |
| T040 | Java-assert im Test statt Assertion-Methode ★ | 2 | ⚠️ | errorprone:UseCorrectAssertInTests, pmd:AssertStatementInTest | `assert x == 1;` → `assertEquals(1, x);` | JUnit 4/5; assert ist ohne -ea wirkungslos |
| T041 | Assertion ohne Meldung | 2 | – | intellij:AssertWithoutMessage, pmd:UnitTestAssertionsShouldIncludeMessage | `assertTrue(valid)` → `assertTrue(valid, "valid")` | JUnit 4/5; umstritten, nicht im Standard |
| T042 | Testmethode ohne Assertion | 2* | – | intellij:TestMethodWithoutAssertion, pmd:UnitTestShouldIncludeAssert | `@Test void t() { service.run(); }` | JUnit 4/5, AssertJ, Mockito-verify; Assertions in Hilfsmethoden nur innerhalb der Datei sichtbar |
| T043 | Zu viele Assertions in einem Test | 2 | – | pmd:UnitTestContainsTooManyAsserts | `@Test void t() { assertX(); assertY(); assertZ(); … }` | JUnit 4/5; Schwellwert konfigurierbar |
| T044 | Kontrollfluss (if/switch/Schleife) in Testmethode | 2 | – | pmd:ControlFlowInTest | `@Test void t() { if (x) assertTrue(y); }` | JUnit 4/5 |
| T045 | Testklasse ohne Testmethoden | 2* | – | intellij:TestCaseWithNoTestMethods, pmd:TestClassWithoutTestCases | `class FooTest { @BeforeEach void setUp() {} }` | JUnit 4/5; nur ohne extends (geerbte Tests unsichtbar) |
| T046 | Testklasse mit Initialisierungslogik im Konstruktor | 2 | – | intellij:TestCaseWithConstructor | `FooTest() { db = connect(); }` → `@BeforeEach` | JUnit 4/5 |
| T047 | Deaktivierter Test (@Ignore/@Disabled) | 2 | – | intellij:IgnoredJUnitTest | `@Disabled @Test void t() {}` | JUnit 4/5 |
| T048 | Testmethode deklariert mehrere Exceptions | 2 | ✅ | intellij:MultipleExceptionsDeclaredOnTestMethod | `@Test void t() throws IOException, SQLException` → `throws Exception` | JUnit 4/5; Stilregel, kollidiert mit A010 |
| T049 | AssertJ: Methodenergebnis statt passender Assertion geprüft ★ | 2* | ✅ | picnic:AssertJDurationRules.AssertThatIsZero+AssertThatIsPositive+AssertThatIsNegative, picnic:AssertJFileRules.AssertThatExists+AssertThatDoesNotExist+AssertThatIsFile+AssertThatIsDirectory+AssertThatIsAbsolute+AssertThatIsRelative+AssertThatIsReadable+AssertThatIsWritable+AssertThatIsExecutable+AssertThatHasFileName+AssertThatHasParentFile+AssertThatHasParentString+AssertThatHasNoParent+AssertThatHasExtension, picnic:AssertJInstantRules.AssertThatIsAfter+AssertThatIsBeforeOrEqualTo+AssertThatIsBefore+AssertThatIsAfterOrEqualTo, picnic:AssertJIterableRules.AssertThatIsEmpty+AssertThatIsNotEmpty+AssertThatContains+AssertThatDoesNotContain+AssertThatContainsAll, picnic:AssertJIteratorRules.AssertThatHasNext+AssertThatIsExhausted, picnic:AssertJMapRules.AssertThatIsEmpty+AssertThatIsNotEmpty+AssertThatHasSize+AssertThatContainsKey+AssertThatDoesNotContainKey+AssertThatContainsValue+AssertThatDoesNotContainValue+AssertThatContainsOnlyKeysObject+AssertThatContainsOnlyKeysIterable, picnic:AssertJObjectRules.AssertThatIsInstanceOfClass+AssertThatIsInstanceOf+AssertThatIsNotInstanceOfClass+AssertThatIsEqualTo+AssertThatIsNotEqualTo+AssertThatHasToString+AssertThatHasSameHashCodeAs, picnic:AssertJObjectRules.AssertThatIsSameAs+AssertThatIsNotSameAs, picnic:AssertJOptionalRules.AssertThatIsPresent+AssertThatIsEmpty+AbstractOptionalAssertIsPresent+AbstractOptionalAssertIsEmpty, picnic:AssertJPathRules.AssertThatExists+AssertThatDoesNotExist+AssertThatIsRegularFile+AssertThatIsDirectory+AssertThatIsSymbolicLink+AssertThatIsReadable+AssertThatIsWritable+AssertThatIsExecutable, picnic:AssertJPrimitiveRules.AssertThatIsLessThan+AssertThatIsLessThanOrEqualTo+AssertThatIsGreaterThan+AssertThatIsGreaterThanOrEqualTo, picnic:AssertJStreamRules.AssertThatNoneMatch+AssertThatAllMatch+AssertThatAnyMatch, picnic:AssertJStringRules.AbstractStringAssertIsEmpty+AbstractStringAssertIsNotEmpty+AssertThatStartsWith+AssertThatDoesNotStartWith+AssertThatEndsWith+AssertThatDoesNotEndWith+AssertThatContains+AssertThatDoesNotContain+AssertThatIsEqualToIgnoringCase+AssertThatIsNotEqualToIgnoringCase+AssertThatIsBlank+AssertThatIsNotBlank+AssertThatMatches+AssertThatDoesNotMatch, picnic:AssertJCharSequenceRules.AssertThatIsEmpty+AssertThatIsNotEmpty, picnic:AssertJComparableRules.AssertThatIsEqualByComparingTo+AssertThatIsNotEqualByComparingTo+AssertThatIsLessThan+AssertThatIsLessThanOrEqualTo+AssertThatIsGreaterThan+AssertThatIsGreaterThanOrEqualTo | `assertThat(l.contains(x)).isTrue()` → `assertThat(l).contains(x)` | AssertJ; Stufe 1 für Methodenmuster (contains, isEmpty, startsWith, compareTo …), File/Path/== nur bei lokal bekanntem Typ |
| T050 | AssertJ: Größe über length/size() statt hasSize… | 2* | ✅ | picnic:AssertJArrayRules.AssertThatHasSize+AssertThatHasSizeLessThan+AssertThatHasSizeLessThanOrEqualTo+AssertThatHasSizeGreaterThan+AssertThatHasSizeGreaterThanOrEqualTo+AssertThatHasSizeBetween, picnic:AssertJCharSequenceRules.AssertThatHasSize, picnic:AssertJEnumerableRules.EnumerableAssertHasSize+EnumerableAssertHasSizeLessThan+EnumerableAssertHasSizeLessThanOrEqualTo+EnumerableAssertHasSizeGreaterThan+EnumerableAssertHasSizeGreaterThanOrEqualTo+EnumerableAssertHasSizeBetween+EnumerableAssertHasSameSizeAs | `assertThat(arr.length).isEqualTo(3)` → `assertThat(arr).hasSize(3)` | AssertJ; .size()-Navigation Stufe 1 |
| T051 | AssertJ: hasSize(0) statt isEmpty() | 1 | ✅ | picnic:AssertJEnumerableRules.EnumerableAssertIsEmpty+AssertIsEmpty+EnumerableAssertIsNotEmpty | `assertThat(l).hasSize(0)` → `assertThat(l).isEmpty()` | AssertJ |
| T052 | AssertJ: Boolean-Assertion umständlich | 1 | ✅ | picnic:AssertJBooleanRules.AbstractBooleanAssertIsTrue+AbstractBooleanAssertIsFalse+AssertThatIsTrue+AssertThatIsFalse+AbstractBooleanAssertIsEqualTo+AbstractBooleanAssertIsNotEqualTo | `assertThat(b).isEqualTo(true)` → `assertThat(b).isTrue()` | AssertJ |
| T053 | AssertJ: Null-Prüfung umständlich | 1 | ✅ | picnic:AssertJNullnessAssertion | `assertThat(x).isEqualTo(null)` → `assertThat(x).isNull()` | AssertJ |
| T054 | AssertJ: isCloseTo mit Toleranz 0 statt isEqualTo | 1 | ✅ | picnic:AssertJByteRules.AbstractByteAssertIsEqualTo+AbstractByteAssertIsNotEqualTo, picnic:AssertJDoubleRules.AbstractDoubleAssertIsEqualTo+AbstractDoubleAssertIsNotEqualTo, picnic:AssertJFloatRules.AbstractFloatAssertIsEqualTo+AbstractFloatAssertIsNotEqualTo, picnic:AssertJIntegerRules.AbstractIntegerAssertIsEqualTo+AbstractIntegerAssertIsNotEqualTo, picnic:AssertJLongRules.AbstractLongAssertIsEqualTo+AbstractLongAssertIsNotEqualTo, picnic:AssertJShortRules.AbstractShortAssertIsEqualTo+AbstractShortAssertIsNotEqualTo | `isCloseTo(x, offset(0.0))` → `isEqualTo(x)` | AssertJ |
| T055 | AssertJ: Vorzeichen/Parität über Vergleich statt isPositive/isOdd | 1 | ✅ | picnic:AssertJNumberRules.NumberAssertIsPositive+NumberAssertIsNotPositive+NumberAssertIsNegative+NumberAssertIsNotNegative+AssertThatIsOdd+AssertThatIsEven | `assertThat(x).isGreaterThan(0)` → `assertThat(x).isPositive()` | AssertJ |
| T056 | AssertJ: Optional-Assertion umständlich | 2* | ✅ | picnic:AssertJOptionalAssertion | `assertThat(opt.get()).isEqualTo(v)` → `assertThat(opt).hasValue(v)` | AssertJ |
| T057 | AssertJ: Files.readString im assertThat statt content() | 2 | ✅ | picnic:AssertJStringRules.AssertThatContent+AssertThatContentUtf8 | `assertThat(Files.readString(p))` → `assertThat(p).content(UTF_8)` | AssertJ |
| T058 | AssertJ: Meldung vorformatiert statt Format-Argumente | 1 | ✅ | picnic:AssertJThrowingCallableRules.AbstractThrowableAssertHasMessage+AbstractThrowableAssertWithFailMessage | `hasMessage("x %s".formatted(a))` → `hasMessage("x %s", a)` | AssertJ |
| T059 | @Mock-Feld manuell zugewiesen | 2 | ✅ | errorprone:UnnecessaryAssignment | `@Mock Foo foo = mock(Foo.class);` → `@Mock Foo foo;` | Mockito |
| T060 | @Mock-Feld ist static ★ | 1 | ✅ | errorprone:StaticMockMember | `@Mock static Foo foo;` → `@Mock Foo foo;` | Mockito |
| T061 | Methode direkt auf Mock aufgerufen | 2 | – | errorprone:DirectInvocationOnMock | `@Mock Foo foo; … foo.bar();` | Mockito; nur außerhalb von when/verify |
| T062 | verify/when ohne Methodenaufruf ★ | 2 | – | errorprone:MockitoUsage | `verify(mock);` → `verify(mock).f();` | Mockito |
| T063 | thenThrow() ohne Argumente ist wirkungslos | 2 | – | errorprone:MisleadingEmptyVarargs | `when(m.f()).thenThrow()` | Mockito |
| T064 | doReturn/when statt when/thenReturn | 2 | ⚠️ | errorprone:MockitoDoSetup | `doReturn(x).when(m).f()` → `when(m.f()).thenReturn(x)` | Mockito; bei Spies nicht äquivalent |
| T065 | Überflüssiges Class-Argument bei mock()/spy() | 2 | ✅ | picnic:MockitoMockClassReference | `Foo f = mock(Foo.class);` → `Foo f = mock();` | Mockito ab 4.10 |
| T066 | Überflüssiges eq() wenn alle Argumente eq() sind | 2 | ✅ | picnic:MockitoStubbing | `when(m.f(eq(1), eq(2)))` → `when(m.f(1, 2))` | Mockito |
| T067 | times(1)/times(0)/getArguments()[i] statt verify/never/getArgument | 2 | ✅ | picnic:MockitoRules.Never+Verify+InvocationOnMockGetArgument | `verify(m, times(1))` → `verify(m)` | Mockito |

### L: Logging (neu)

| Code | Regel | Stufe | Fix | Herkunft | Beispiel | Hinweis |
|---|---|---|---|---|---|---|
| L001 | Logger-Feld nicht private static final | 2 | ✅ | pmd:ProperLogger, picnic:Slf4jLoggerDeclaration, intellij:NonStaticFinalLogger, baseline:PreferStaticLoggers, netbeans:Logger declaration is not static final (logging) | `private Logger log = LoggerFactory.getLogger(A.class);` → `private static final Logger log` | SLF4J, Log4j 2, JUL; PMD ProperLogger prüft auch die Klasse (→ L002) |
| L002 | Logger mit Klassenliteral einer fremden Klasse erzeugt ★ | 2 | ✅ | intellij:LoggerInitializedWithForeignClass, baseline:LoggerEnclosingClass | `class A { Logger log = LoggerFactory.getLogger(B.class); }` → `A.class` | SLF4J, Log4j 2, JUL (getLogger(B.class.getName())) |
| L003 | Mehrere Logger in einer Klasse | 2 | – | pmd:MoreThanOneLogger, intellij:ClassWithMultipleLoggers, netbeans:Multiple loggers (logging) | `static final Logger a = …; static final Logger b = …;` | SLF4J, Log4j 2, JUL |
| L004 | Anzahl {}-Platzhalter passt nicht zu den Argumenten ★ | 2* | – | intellij:LoggingPlaceholderCountMatchesArgumentCount, picnic:Slf4jLogStatement, pmd:InvalidLogMessageFormat | `log.info("a={} b={}", a)` | SLF4J, Log4j 2; letztes Argument zählt nicht, wenn lokal als Throwable bekannt |
| L005 | Platzhalter verbraucht die Exception (kein Stacktrace) | 2 | ⚠️ | baseline:LoggerInterpolationConsumesThrowable | `catch (E e) { log.warn("failed {}", e); }` → `log.warn("failed", e)` | SLF4J, Log4j 2; Exception als catch-Parameter bekannt |
| L006 | Exception nicht als letztes Log-Argument ★ | 2 | ✅ | baseline:Slf4jThrowable | `log.warn("x {}", e, id)` → `log.warn("x {}", id, e)` | SLF4J, Log4j 2 |
| L007 | Log im catch-Block ohne die gefangene Exception | 2 | ⚠️ | baseline:CatchBlockLogException | `catch (IOException e) { log.warn("failed"); }` → `log.warn("failed", e)` | SLF4J, Log4j 2, JUL |
| L008 | Log-Nachricht vorab formatiert/verkettet statt Platzhalter | 2 | ⚠️ | intellij:StringConcatenationArgumentToLogCall, netbeans:String concatenation in logger (logging), picnic:FormatStringConcatenation, baseline:Slf4jConstantLogMessage, picnic:EagerStringFormatting | `log.info("x=" + x)` → `log.info("x={}", x); log.info(String.format("x=%s", x))` → `log.info("x={}", x)` | SLF4J, Log4j 2; picnic auch für Guava Preconditions |
| L009 | Log-Guard passt nicht zum Level des Aufrufs ★ | 2 | ⚠️ | intellij:LoggingConditionDisagreesWithLogLevelStatement, baseline:Slf4jLevelCheck | `if (log.isDebugEnabled()) { log.info(…); }` → `isInfoEnabled()` | SLF4J, Log4j 2, JUL (isLoggable) |
| L010 | Log-Guard fehlt bzw. ist überflüssig (wählbar) | 2* | ⚠️ | intellij:LogStatementNotGuardedByLogCondition, pmd:GuardLogStatement, intellij:LoggingGuardedByCondition | `log.debug("x=" + compute())` → `if (log.isDebugEnabled()) …` | SLF4J, Log4j 2, JUL; Option: Guard verlangen (teure Argumente) oder überflüssigen Guard bei reinen Platzhaltern melden (Gegenregeln) |
| L011 | Ähnliche Log-Nachrichten in einer Klasse | 2* | – | intellij:LoggingSimilarMessage | `log.info("loaded: {}", a); … log.info("loaded: {}", b);` | SLF4J, Log4j 2, JUL |

### K: Nebenläufigkeit (neu)

| Code | Regel | Stufe | Fix | Herkunft | Beispiel | Hinweis |
|---|---|---|---|---|---|---|
| K001 | Thread.run() statt start() ★ | 2* | ⚠️ | intellij:ThreadRun, netbeans:Invoking Thread.run() (thread), pmd:DontCallThreadRun | `new Thread(r).run()` → `new Thread(r).start()` | nur bei new Thread(…) oder lokal als Thread deklariert |
| K002 | Thread erzeugt, aber nie gestartet ★ | 2 | – | errorprone:DeadThread | `new Thread(r);` |  |
| K003 | Thread ohne Runnable und ohne überschriebenes run() | 1 | – | intellij:ThreadWithDefaultRunMethod | `new Thread().start();` |  |
| K004 | Klasse erweitert Thread statt Runnable | 2 | – | intellij:ExtendsThread, pmd:OverridingThreadRun | `class Worker extends Thread { public void run() {…} }` |  |
| K005 | Thread im Konstruktor gestartet | 2* | – | intellij:ThreadStartInConstruction, netbeans:Starting Thread in constructor (thread) | `Foo() { new Thread(r).start(); }` |  |
| K006 | Thread.stop/suspend/resume aufgerufen ★ | 2* | – | intellij:ThreadStopSuspendResume, netbeans:Invoking Thread.stop()/suspend()/resume() (thread) | `Thread t; t.stop();` | stop() wirft ab Java 20 UnsupportedOperationException |
| K007 | Thread.yield/setPriority verwendet | 2* | – | errorprone:ThreadPriorityCheck, intellij:ThreadYield, netbeans:Invoking Thread.yield() (thread), intellij:ThreadPriority | `Thread.yield(); t.setPriority(10);` | Thread.yield() Stufe 1 |
| K008 | ThreadGroup verwendet | 2 | – | pmd:AvoidThreadGroup | `new ThreadGroup("g")` |  |
| K009 | Thread.Builder.name mit Formatplatzhalter | 2 | – | errorprone:ThreadBuilderNameWithPlaceholder | `Thread.ofVirtual().name("w-%d")` → `name("w-", 0)` | ab 21 |
| K010 | Thread.interrupted() statt interrupt() im catch ★ | 2 | ✅ | errorprone:InterruptedInCatchBlock | `catch (InterruptedException e) { Thread.interrupted(); }` → `Thread.currentThread().interrupt()` |  |
| K011 | Future.cancel(true) ohne Grund | 2* | ⚠️ | errorprone:Interruption | `future.cancel(true)` → `future.cancel(false)` |  |
| K012 | Thread.join nicht in Schleife bei InterruptedException | 2* | – | errorprone:ThreadJoinLoop | `try { t.join(); } catch (InterruptedException e) {}` |  |
| K013 | Thread.sleep() in Schleife (Busy-Wait) | 2 | – | intellij:BusyWait, netbeans:Thread.sleep in loop (thread) | `while (!done) { Thread.sleep(100); }` |  |
| K014 | Warteschleife auf nicht-volatiles Feld | 2 | – | intellij:WhileLoopSpinsOnField | `private boolean done; … while (!done) {}` |  |
| K015 | Thread.sleep() bei gehaltenem Lock | 1 | – | intellij:SleepWhileHoldingLock, netbeans:Thread.sleep in synchronized context (thread) | `synchronized (m) { Thread.sleep(100); }` |  |
| K016 | wait()/await() nicht in Schleife bzw. ohne Bedingung ★ | 2* | – | errorprone:WaitNotInLoop, intellij:WaitNotInLoop, intellij:AwaitNotInLoop, intellij:UnconditionalWait | `if (!ready) m.wait();` → `while (!ready) m.wait();` | wait() Stufe 1; await() nur bei lokal als Condition deklarierter Variable (CountDownLatch.await braucht keine Schleife) |
| K017 | wait()/notify() außerhalb von synchronized | 1 | – | intellij:WaitNotifyNotInSynchronizedContext, netbeans:.notify invoked outside a synchronized context (thread), netbeans:.wait invoked outside a synchronized context (thread) | `void f() { lock.notify(); }` | Fehlalarm möglich bei Hilfsmethoden, die nur unter Lock aufgerufen werden |
| K018 | wait()/await() ohne Timeout | 1 | – | intellij:WaitOrAwaitWithoutTimeout | `m.wait();` → `m.wait(1000);` |  |
| K019 | wait() bei zwei gehaltenen Locks | 1 | – | intellij:WaitWhileHoldingTwoLocks | `synchronized (a) { synchronized (b) { b.wait(); } }` |  |
| K020 | notify()/signal() statt notifyAll()/signalAll() | 2* | ⚠️ | intellij:ObjectNotify, pmd:UseNotifyAllInsteadOfNotify, intellij:ConditionSignal | `lock.notify()` → `lock.notifyAll()` | notify() Stufe 1; signal() nur bei lokal als Condition deklariert |
| K021 | notify()/notifyAll() ohne vorherige Zustandsänderung | 1 | – | intellij:NakedNotify | `synchronized (this) { notifyAll(); }` |  |
| K022 | wait/notify bzw. await/signal ohne Gegenstück in der Klasse | 2 | – | intellij:WaitWithoutCorrespondingNotify, intellij:NotifyWithoutCorrespondingWait, intellij:AwaitWithoutCorrespondingSignal, intellij:SignalWithoutCorrespondingAwait | `lock.wait(); // nirgends lock.notify()` | nur private Felder sicher |
| K023 | wait()/notify() auf Condition statt await()/signal() ★ | 2* | ⚠️ | netbeans:Invoking Condition.notify() (thread), netbeans:Invoking Condition.wait() (thread), intellij:NotifyCalledOnCondition, intellij:WaitCalledOnCondition | `Condition c = lock.newCondition(); c.wait();` → `c.await();` |  |
| K024 | Lock ohne unmittelbar folgendes try/finally-unlock ★ | 2* | – | intellij:SafeLock, netbeans:Lock not unlocked in finally (thread), errorprone:LockNotBeforeTry | `lock.lock(); work(); lock.unlock();` → `lock.lock(); try { work(); } finally { lock.unlock(); }` |  |
| K025 | synchronized auf java.util.concurrent.locks.Lock | 2* | – | intellij:SynchronizeOnLock, netbeans:Synchronizing on Lock (thread) | `final Lock lock = new ReentrantLock(); synchronized (lock) {…}` → `lock.lock()` |  |
| K026 | synchronized auf nicht-finales Feld ★ | 2 | – | intellij:SynchronizeOnNonFinalField, errorprone:SynchronizeOnNonFinalField, netbeans:Synchronization on non-final field (thread) | `private Object lock; synchronized (lock) {…}` |  |
| K027 | synchronized auf Wrapper/wertbasierte Klasse ★ | 2* | – | errorprone:LockOnBoxedPrimitive, intellij:SynchronizeOnValueBasedClass, javac:compiler.warn.attempt.to.synchronize.on.instance.of.value.based.class | `private final Integer lock = 0; synchronized (lock) {}` |  |
| K028 | synchronized auf mit Literal initialisiertes Objekt | 2 | – | intellij:SynchronizedOnLiteralObject | `private final String lock = "LOCK"; synchronized (lock) {…}` | interne Strings werden geteilt |
| K029 | synchronized auf lokale Variable oder Parameter | 2 | – | intellij:SynchronizationOnLocalVariableOrMethodParameter | `Object l = new Object(); synchronized (l) {…}` |  |
| K030 | synchronized auf getClass() | 1 | ⚠️ | intellij:SynchronizationOnGetClass | `synchronized (getClass())` → `synchronized (Foo.class)` |  |
| K031 | synchronized auf fremdes Klassenliteral | 1 | – | errorprone:LockOnNonEnclosingClassLiteral | `synchronized (Other.class)` |  |
| K032 | synchronized(this) in anonymer/lokaler Klasse | 1 | – | netbeans:Synchronizing on this in anonymous class (bugs) | `new Runnable() { public void run() { synchronized (this) {…} } }` |  |
| K033 | synchronized auf this oder Klassenliteral | 1 | – | intellij:SynchronizeOnThis | `synchronized (this) {…}` → `private final Object lock` | Stilregel |
| K034 | synchronized auf statisches Feld | 2 | – | intellij:SynchronizationOnStaticField | `static final Object LOCK = …; synchronized (LOCK) {…}` | Stilregel |
| K035 | synchronized-Modifier an Methode | 1 | – | intellij:SynchronizedMethod, pmd:AvoidSynchronizedAtMethodLevel | `public synchronized void f()` → `synchronized (lock) {…}` | Stilregel |
| K036 | synchronized-Block (Pinning bei virtuellen Threads) | 1 | – | pmd:AvoidSynchronizedStatement | `synchronized (lock) {…}` → `ReentrantLock` | nur 21 bis 23 relevant (ab 24 kein Pinning) |
| K037 | Verschachtelte synchronized-Blöcke (Deadlock-Gefahr) | 1 | – | intellij:NestedSynchronizedStatement, netbeans:Nested synchronized blocks (thread) | `synchronized (a) { synchronized (b) {…} }` |  |
| K038 | Aufruf einer native-Methode unter Lock | 2 | – | intellij:CallToNativeMethodWhileLocked | `synchronized (lock) { nativeRead(); }` | nur native-Methoden der eigenen Datei |
| K039 | Nicht synchronisierte Überschreibung einer synchronisierten Methode | 2* | – | errorprone:UnsynchronizedOverridesSynchronized | `class B extends A { void f() {} } bei synchronized A.f()` | nur Oberklassen in derselben Datei |
| K040 | Feld teils synchronisiert, teils unsynchronisiert verwendet | 2 | – | intellij:FieldAccessedSynchronizedAndUnsynchronized | `synchronized void a() { n++; } void b() { n = 0; }` |  |
| K041 | Nicht-privates Feld in synchronized-Kontext | 2 | – | intellij:PublicFieldAccessedInSynchronizedContext | `protected int n; synchronized void inc() { n++; }` |  |
| K042 | Statisches Feld unter Instanz-Lock verändert ★ | 2 | – | intellij:AccessToStaticFieldLockedOnInstance, errorprone:StaticGuardedByInstance | `static int n; synchronized void inc() { n++; }` |  |
| K043 | Nicht atomare Operation auf volatile-Feld ★ | 2 | – | intellij:NonAtomicOperationOnVolatileField, errorprone:NonAtomicVolatileUpdate | `volatile int n; n++;` |  |
| K044 | volatile-Array-Feld (Elemente nicht volatile) | 1 | – | intellij:VolatileArrayField, netbeans:Volatile array field (thread) | `private volatile int[] data;` |  |
| K045 | Nicht threadsichere Lazy-Initialisierung | 2 | – | pmd:NonThreadSafeSingleton, intellij:NonThreadSafeLazyInitialization | `static Foo f; static Foo get() { if (f == null) f = new Foo(); return f; }` | Gegenstück zu B019 |
| K046 | Statische Initialisierung referenziert Unterklasse (Deadlock) | 2 | – | errorprone:ClassInitializationDeadlock, intellij:StaticInitializerReferencesSubClass | `class A { static final A X = new B(); } class B extends A {}` | nur Unterklassen in derselben Datei |
| K047 | Nicht threadsicherer Formatter als statisches Feld ★ | 2 | – | errorprone:DateFormatConstant, pmd:UnsynchronizedStaticFormatter, intellij:AccessToNonThreadSafeStaticFieldFromInstance | `static final SimpleDateFormat F = new SimpleDateFormat("yyyy");` → `DateTimeFormatter` | SimpleDateFormat, DateFormat, MessageFormat … |
| K048 | MessageDigest als Feld (nicht threadsicher) | 2 | – | pmd:AvoidMessageDigestField | `private final MessageDigest md = MessageDigest.getInstance("SHA-256");` |  |
| K049 | ThreadLocal-Feld nicht static final | 2 | ⚠️ | errorprone:ThreadLocalUsage, intellij:ThreadLocalNotStaticFinal | `private ThreadLocal<X> tl = …` → `private static final ThreadLocal<X> tl` |  |
| K050 | ThreadLocal.set(null) statt remove() | 2* | ✅ | intellij:ThreadLocalSetWithNull | `tl.set(null);` → `tl.remove();` |  |
| K051 | ThreadLocalRandom.current() gespeichert oder weitergegeben | 2 | – | intellij:SharedThreadLocalRandom | `private Random r = ThreadLocalRandom.current();` |  |
| K052 | AtomicXFieldUpdater-Feld nicht static final | 2 | ⚠️ | intellij:AtomicFieldUpdaterNotStaticFinal | `AtomicIntegerFieldUpdater<Foo> u = …` → `private static final` |  |
| K053 | AtomicXFieldUpdater auf fehlendes/nicht volatiles Feld | 2 | – | intellij:AtomicFieldUpdaterIssues | `AtomicIntegerFieldUpdater.newUpdater(Foo.class, "cnt") bei int cnt` |  |
| K054 | compareAndSet auf AtomicReference mit Wrappertyp | 2* | – | errorprone:PrimitiveAtomicReference | `AtomicReference<Integer> r; r.compareAndSet(1, 2)` |  |
| K055 | CompletableFuture-Async ohne Executor | 2 | – | baseline:DangerousCompletableFutureUsage | `CompletableFuture.supplyAsync(s)` → `supplyAsync(s, executor)` |  |
| K056 | Future von submit() verworfen (Exceptions gehen verloren) | 2* | ⚠️ | baseline:ExecutorSubmitRunnableFutureIgnored | `executor.submit(r);` → `executor.execute(r);` |  |
| K057 | ThreadPoolExecutor mit unbeschränkter Queue und maxPoolSize | 2 | – | errorprone:ErroneousThreadPoolConstructorChecker | `new ThreadPoolExecutor(1, 10, …, new LinkedBlockingQueue<>())` |  |
| K058 | ScheduledThreadPoolExecutor mit 0 Kern-Threads ★ | 2 | – | intellij:ScheduledThreadPoolExecutorWithZeroCoreThreads | `new ScheduledThreadPoolExecutor(0)` |  |
| K059 | parallel()/parallelStream() verwendet | 1 | – | baseline:DangerousParallelStreamUsage | `list.parallelStream()` |  |
| K060 | Collector.of mit zustandsbehafteter anonymer Klasse | 2 | – | errorprone:CollectorShouldNotUseState | `Collector.of(new Supplier<>() { int n; … }, …)` |  |
| K061 | AsyncCallable/AsyncFunction gibt null statt Future zurück | 2 | ⚠️ | errorprone:AsyncCallableReturnsNull, errorprone:AsyncFunctionReturnsNull | `new AsyncCallable<T>() { … return null; }` → `immediateFuture(null)` | Guava |
| K062 | transformAsync mit immediateFuture statt transform | 2 | ✅ | errorprone:FutureTransformAsync | `Futures.transformAsync(f, x -> immediateFuture(g(x)), e)` → `transform(f, x -> g(x), e)` | Guava |
| K063 | Reference.reachabilityFence nicht im finally | 2 | – | errorprone:ReachabilityFenceUsage | `Reference.reachabilityFence(o); außerhalb von finally` |  |
| K064 | Methode besteht nur aus synchronized(this)-Block | 1 | ✅ | intellij:MethodMayBeSynchronized | `void f() { synchronized (this) { … } }` → `synchronized void f() { … }` |  |

### V: Sicherheit (neu)

| Code | Regel | Stufe | Fix | Herkunft | Beispiel | Hinweis |
|---|---|---|---|---|---|---|
| V001 | Java-Deserialisierung verwendet | 2* | – | errorprone:BanSerializableRead, baseline:DangerousJavaDeserialization | `new ObjectInputStream(in).readObject()` |  |
| V002 | JNDI-Lookup | 2* | – | errorprone:BanJNDI | `new InitialContext().lookup(name)` |  |
| V003 | Runtime.exec() (wählbar: nur mit nicht-konstantem Befehl) | 2 | – | intellij:RuntimeExec, intellij:RuntimeExecWithNonConstantString | `Runtime.getRuntime().exec("ls " + dir)` | Option: jeden Aufruf oder nur nicht-konstante Befehle melden |
| V004 | System.loadLibrary/load (wählbar: nur mit dynamischem Namen) | 2 | – | pmd:AvoidUsingNativeCode, intellij:LoadLibraryWithNonConstantString | `System.loadLibrary(name + "64")` | Option: jeden Aufruf oder nur nicht-konstante Namen melden |
| V005 | SQL-Anweisung aus zusammengesetztem String | 2* | – | intellij:JDBCExecuteWithNonConstantString, intellij:JDBCPrepareStatementWithNonConstantString | `stmt.execute("DELETE FROM t WHERE id=" + id)` | nur Statement/Connection mit lokal bekanntem Typ; Konstanten-Verkettung nicht melden |
| V006 | ClassLoader erzeugt oder abgeleitet | 2* | – | intellij:ClassLoaderInstantiation, errorprone:BanClassLoader, intellij:CustomClassloader | `new URLClassLoader(urls); class MyLoader extends ClassLoader` |  |
| V007 | Zugriffsschutz per setAccessible(true) aufgehoben | 1 | – | pmd:AvoidAccessibilityAlteration, forbidden-apis:jdk-reflection | `field.setAccessible(true);` |  |
| V008 | Hartkodierter Kryptoschlüssel ★ | 2 | – | pmd:HardCodedCryptoKey | `new SecretKeySpec("secret".getBytes(), "AES")` |  |
| V009 | Hartkodierter Initialisierungsvektor | 2 | – | pmd:InsecureCryptoIv | `new IvParameterSpec(new byte[16])` |  |
| V010 | Unsicherer Krypto-Modus | 2 | – | errorprone:InsecureCryptoUsage | `Cipher.getInstance("AES")` → `"AES/GCM/NoPadding"` | nur String-Literale |
| V011 | java.util.Random/Math.random() statt SecureRandom | 2 | – | intellij:UnsecureRandomNumberGeneration | `new Random().nextInt()` | nur in sicherheitsrelevantem Code sinnvoll |
| V012 | Hartkodierte IP-Adresse | 1 | – | pmd:AvoidUsingHardCodedIP | `String host = "192.168.0.1";` |  |
| V013 | Klonbare Klasse in sicherem Kontext | 2* | – | intellij:CloneableClassInSecureContext | `class Secret implements Cloneable {…}` | vgl. B034 |
| V014 | Unicode-Richtungssteuerzeichen im Quelltext ★ | 1 | – | errorprone:UnicodeDirectionalityCharacters | `"\u202E" als Rohzeichen` | Trojan Source |

## 4. Durch bestehende jtk-Regeln abgedeckt

Diese Regeln der Tools entsprechen (ganz oder im Wesentlichen) einer bestehenden jtk-Regel. Sie eignen sich als zusätzliche Testfälle und für die Unterdrückungs-Aliase (`@SuppressWarnings`) beim Config-Import. Die Zeilen mit verworfenen Codes wurden bewusst nicht umgesetzt (regel-analyse.md, Abschnitt 5); `D` und `X` sind die dort geplanten Gruppen ohne Codes.

| jtk | Entsprechungen |
|---|---|
| I001 | intellij:OnDemandImport, errorprone:WildcardImport, pmd:OnDemandImport, netbeans:Star import (imports) |
| I002 | intellij:UseOfSunClasses, errorprone:SunApi, pmd:DontImportSun, pmd:UnsupportedJdkApiUsage, netbeans:Import from Excluded (imports), forbidden-apis:jdk-internal, javac:compiler.warn.sun.proprietary |
| I003 | intellij:StaticImport, pmd:TooManyStaticImports, picnic:NonStaticImport, picnic:StaticImport |
| I004 | pmd:LoosePackageCoupling |
| I005 | intellij:UnnecessaryFullyQualifiedName, errorprone:UnnecessarilyFullyQualified, pmd:UnnecessaryFullyQualifiedName, baseline:UnnecessarilyQualified |
| B001 | intellij:EqualsAndHashcode, errorprone:EqualsHashCode, pmd:OverrideBothEqualsAndHashcode, netbeans:Generate missing hashCode or equals (general), javac:compiler.warn.override.equals.but.not.hashcode, eclipse:COMPILER_PB_MISSING_HASHCODE_METHOD |
| B002 | intellij:CovariantEquals, errorprone:NonOverridingEquals |
| B003 | intellij:CatchMayIgnoreException, errorprone:EmptyCatch, pmd:EmptyCatchBlock |
| B004 | intellij:FallthruInSwitchStatement, errorprone:FallThrough, pmd:ImplicitSwitchFallThrough, javac:fallthrough, eclipse:COMPILER_PB_FALLTHROUGH_CASE |
| B005 | intellij:SwitchStatementsWithoutDefault, errorprone:MissingDefault, pmd:NonExhaustiveSwitch, eclipse:COMPILER_PB_SWITCH_MISSING_DEFAULT_CASE |
| B006 | intellij:DefaultNotLastCaseInSwitch, errorprone:SwitchDefault, pmd:DefaultLabelNotLastInSwitch |
| B007 | intellij:StringEquality, pmd:UseEqualsToCompareStrings, netbeans:Comparing Strings using == or != (general) |
| B008 | intellij:LiteralAsArgToStringEquals, pmd:LiteralsFirstInComparisons |
| B009 | errorprone:EqualsNull, pmd:EqualsNull, netbeans:.equals(null) (bugs) |
| B010 | intellij:ContinueOrBreakFromFinallyBlock, intellij:FinallyBlockCannotCompleteNormally, intellij:ReturnFromFinallyBlock, intellij:ThrowFromFinallyBlock, errorprone:Finally, pmd:DoNotThrowExceptionInFinally, pmd:ReturnFromFinallyBlock, netbeans:'finally' block suppresses exceptions (bugs), netbeans:'throw' inside 'finally' block (bugs), javac:finally, eclipse:COMPILER_PB_FINALLY_BLOCK_NOT_COMPLETING |
| B011 | intellij:ExpressionComparedToItself, intellij:SillyAssignment, errorprone:IdentityBinaryExpression, errorprone:SelfAssignment, errorprone:SelfComparison, errorprone:SelfEquals, pmd:IdempotentOperations, netbeans:Assignment To Itself (general), eclipse:COMPILER_PB_COMPARING_IDENTICAL, eclipse:COMPILER_PB_NO_EFFECT_ASSIGNMENT |
| B012 | intellij:AssignmentUsedAsCondition, intellij:NestedAssignment, errorprone:AssignmentExpression, errorprone:LogicalAssignment, pmd:AssignmentInOperand, netbeans:Nested assignment (assignment_issues), eclipse:COMPILER_PB_POSSIBLE_ACCIDENTAL_BOOLEAN_ASSIGNMENT |
| B013 | intellij:AssignmentToLambdaParameter, intellij:AssignmentToMethodParameter, intellij:ParameterCanBeLocal, pmd:AvoidReassigningParameters, netbeans:Assignment to method parameter (assignment_issues), eclipse:COMPILER_PB_PARAMETER_ASSIGNMENT |
| B014 | intellij:AssignmentToForLoopParameter, errorprone:IncrementInForLoopAndHeader, pmd:AvoidReassigningLoopVariables, netbeans:Assignment to for-loop parameter (assignment_issues) |
| B015 | pmd:JumbledIncrementer |
| B016 | pmd:BrokenNullCheck, pmd:MisplacedNullCheck |
| B017 | intellij:UnpredictableBigDecimalConstructorCall, errorprone:BigDecimalLiteralDouble, pmd:AvoidDecimalLiteralsInBigDecimalConstructor |
| B018 | intellij:ConfusingOctalEscape, pmd:SuspiciousOctalEscape |
| B019 | intellij:DoubleCheckedLocking, errorprone:DoubleCheckedLocking, pmd:DoubleCheckedLocking, netbeans:Double-checked locking (thread) |
| B020 | intellij:BadExceptionCaught, pmd:AvoidCatchingGenericException, pmd:AvoidCatchingNPE, pmd:AvoidCatchingThrowable |
| B021 | intellij:BadExceptionThrown, errorprone:ThrowSpecificExceptions, pmd:AvoidThrowingRawExceptionTypes |
| B022 | intellij:SystemOutErr, intellij:ThrowablePrintStackTrace, intellij:ThrowablePrintedToSystemOut, errorprone:CatchAndPrintStackTrace, errorprone:SystemOut, pmd:AvoidPrintStackTrace, pmd:SystemPrintln, netbeans:Print Stack Trace (code_maturity), netbeans:System out / err (code_maturity), forbidden-apis:jdk-system-out |
| B023 | intellij:ExceptionFromCatchWhichDoesntWrap, errorprone:UnusedException, pmd:PreserveStackTrace |
| B024 | intellij:CloneCallsConstructors, pmd:FinalizeDoesNotCallSuperFinalize, pmd:ProperCloneImplementation, netbeans:clone() does not call super.clone() (bugs), netbeans:finalize() does not call super.finalize() (finalization) |
| B025 | intellij:DoubleBraceInitialization, errorprone:DoubleBraceInitialization, pmd:DoubleBraceInitialization |
| B026 | intellij:UnusedAssignment, errorprone:UnusedVariable, pmd:UnusedAssignment, pmd:UnusedLocalVariable, netbeans:DN_org.netbeans.modules.java.hints.bugs.Unused (bugs), netbeans:Unused Assignment (bugs), netbeans:Unused Compound Assignment (bugs), baseline:StrictUnusedVariable, eclipse:COMPILER_PB_UNUSED_LOCAL |
| B027 | errorprone:UnusedMethod, pmd:UnusedPrivateField, pmd:UnusedPrivateMethod, eclipse:COMPILER_PB_UNUSED_PRIVATE_MEMBER |
| B028 | pmd:UnusedFormalParameter, eclipse:COMPILER_PB_UNUSED_PARAMETER |
| B029 (verworfen) | intellij:LambdaParameterHidingMemberVariable, intellij:LocalVariableHidingMemberVariable, intellij:ParameterHidingMemberVariable, intellij:PatternVariableHidesField, eclipse:COMPILER_PB_LOCAL_VARIABLE_HIDING |
| B030 | intellij:UnclearBinaryExpression, errorprone:OperatorPrecedence |
| B033 | intellij:Finalize, intellij:FinalizeNotProtected, errorprone:Finalize, pmd:EmptyFinalizer, pmd:FinalizeOnlyCallsSuperFinalize, pmd:FinalizeOverloaded, pmd:FinalizeShouldBeProtected, netbeans:finalize() declared (finalization), netbeans:finalize() not declared protected (finalization) |
| B034 | intellij:UseOfClone |
| S001 | intellij:ControlFlowStatementWithoutBraces, errorprone:MissingBraces, pmd:ControlStatementBraces, netbeans:Do-While Loops Should Use Braces (braces), netbeans:For Loops Should Use Braces (braces), netbeans:If-Else Statements Should Use Braces (braces), netbeans:While Loops Should Use Braces (braces) |
| S002 | intellij:SimplifiableIfStatement, intellij:TrivialIf, pmd:SimplifyBooleanReturns, netbeans:DN_redundantIf (general) |
| S003 | intellij:PointlessBooleanExpression, intellij:SimplifiableConditionalExpression, pmd:SimplifiedTernary, pmd:SimplifyBooleanExpressions, netbeans:DN_redundantConditional (general) |
| S004 | pmd:CollapsibleIfStatements, netbeans:DN_org.netbeans.modules.java.hints.suggestions.Tiny.mergeIfs (suggestions) |
| S005 | intellij:UnnecessaryLocalVariable, pmd:UnnecessaryLocalBeforeReturn, pmd:VariableCanBeInlined, picnic:DirectReturn |
| S006 | intellij:UnnecessaryParentheses, errorprone:UnnecessaryParentheses, pmd:UselessParentheses, baseline:UnnecessaryLambdaArgumentParentheses |
| S007 | intellij:UnnecessarySemicolon, errorprone:EmptyIf, errorprone:EmptyTopLevelDeclaration, errorprone:UnnecessarySemicolon, pmd:UnnecessarySemicolon, netbeans:Empty statement (empty), netbeans:Empty statement after 'do' (empty), netbeans:Empty statement after 'for' (empty), netbeans:Empty statement after 'if/else' (empty), netbeans:Empty statement after 'while' (empty), javac:compiler.warn.extraneous.semicolon, javac:empty, eclipse:COMPILER_PB_EMPTY_STATEMENT |
| S008 | intellij:FinalMethodInFinalClass, intellij:FinalPrivateMethod, intellij:UnnecessaryModifier, pmd:FinalParameterInAbstractMethod, pmd:UnnecessaryModifier, netbeans:Final method in final class (class_structure), netbeans:Final private method (class_structure), baseline:RedundantModifier, javac:strictfp |
| S009 | errorprone:AnnotationPosition, pmd:ModifierOrder |
| S010 | intellij:UnnecessaryReturn, errorprone:ReturnAtTheEndOfVoidFunction, pmd:UnnecessaryReturn, netbeans:DN_org.netbeans.modules.java.hints.RemoveUnnecessaryReturn (general) |
| S011 | intellij:UnnecessarySuperConstructor |
| S012 | intellij:RedundantFieldInitialization, pmd:RedundantFieldInitializer |
| S013 | intellij:MultipleVariablesInDeclaration, errorprone:MultiVariableDeclaration, pmd:OneDeclarationPerLine |
| S014 | intellij:CStyleArrayDeclaration, errorprone:MixedArrayDimensions, pmd:CStyleArrayDeclaration |
| S015 | intellij:LongLiteralsEndingWithLowercaseL, errorprone:LongLiteralLowerCaseSuffix, pmd:LongLiteralEndingWithLowercaseL |
| S017 | intellij:EmptyFinallyBlock, intellij:EmptyInitializer, intellij:EmptyStatementBody, intellij:EmptySynchronizedStatement, intellij:EmptyTryBlock, pmd:EmptyControlStatement, pmd:UnconditionalIfStatement, netbeans:Empty synchronized block (thread), eclipse:COMPILER_PB_DEAD_CODE_IN_TRIVIAL_IF_STATEMENT, eclipse:COMPILER_PB_UNDOCUMENTED_EMPTY_BLOCK |
| S018 | intellij:UnnecessaryBlockStatement, pmd:UnnecessaryBlock |
| S019 | intellij:BooleanConstructor, intellij:CachedNumberConstructorCall, errorprone:BoxedPrimitiveConstructor, pmd:PrimitiveWrapperInstantiation, pmd:StringInstantiation, netbeans:Boxed Primitive instantiation (rules15), netbeans:Creating new Boolean (performance), netbeans:String constructor (performance), picnic:StringRules.EmptyString+StringIdentity, modernizer:Wrapper-Konstruktoren → valueOf |
| S020 | intellij:PointlessNullCheck, pmd:SimplifyConditional |
| S021 | intellij:CodeBlock2Expr |
| M001 | intellij:PatternVariableCanBeUsed, errorprone:PatternMatchingInstanceof, netbeans:DN_ConvertToPatternInstanceOf (rules15) |
| M002 | intellij:EnhancedSwitchMigration, errorprone:StatementSwitchToExpressionSwitch, errorprone:TraditionalSwitchExpression, netbeans:DN_org.netbeans.modules.java.hints.jdk.ConvertSwitchToRuleSwitch (rules15) |
| M004 | intellij:Convert2Diamond, pmd:UseDiamondOperator, netbeans:Can Use Diamond (rules15), javac:compiler.warn.diamond.redundant.args, eclipse:COMPILER_PB_REDUNDANT_TYPE_ARGUMENTS |
| M006 | intellij:RedundantExplicitVariableType, intellij:VariableTypeCanBeExplicit, errorprone:VarWithPrimitive, errorprone:Varifier, pmd:UseExplicitTypes, netbeans:Convert Explicit Type to Var (rules15), netbeans:Convert Var to Explicit Type (rules15), baseline:VarUsage, javac:compiler.warn.local.redundant.type |
| M007 | intellij:ForCanBeForeach, pmd:ForLoopCanBeForeach |
| M008 | errorprone:UnnamedVariable, eclipse:COMPILER_PB_UNUSED_EXCEPTION_PARAMETER, eclipse:COMPILER_PB_UNUSED_LAMBDA_PARAMETER |
| M009 | intellij:TextBlockMigration, errorprone:StringConcatToTextBlock, netbeans:DN_ConvertToTextBlock (rules15) |
| M010 | intellij:TryWithIdenticalCatches, pmd:IdenticalCatchBranches, netbeans:Join catch sections using multicatch (rules15) |
| M011 | pmd:MethodArgumentCouldBeFinal |
| M012 | intellij:CanBeFinal, intellij:FieldMayBeFinal, errorprone:FieldCanBeFinal, pmd:ImmutableField, netbeans:DN_CanBeFinal (thread) |
| M013 | intellij:MissingDeprecatedAnnotation, errorprone:DepAnn, javac:dep-ann, eclipse:COMPILER_PB_MISSING_DEPRECATED_ANNOTATION |
| M014 | intellij:FillPermitsList |
| M015 | intellij:LocalCanBeFinal, pmd:LocalVariableCouldBeFinal |
| N001 | intellij:DollarSignInName, intellij:NewClassNamingConvention, pmd:AvoidDollarSigns, pmd:ClassNamingConventions |
| N002 | intellij:NewMethodNamingConvention, errorprone:IdentifierName, pmd:MethodNamingConventions |
| N003 | intellij:FieldNamingConvention, intellij:UpperCaseFieldNameNotConstant, pmd:FieldNamingConventions |
| N004 | netbeans:Constant naming conventions (naming), picnic:ConstantNaming |
| N006 | intellij:LocalVariableNamingConvention, pmd:LocalVariableNamingConventions |
| N007 | intellij:LambdaParameterNamingConvention, intellij:ParameterNamingConvention, pmd:FormalParameterNamingConventions |
| N011 | errorprone:TypeParameterNaming, pmd:GenericsNaming, pmd:TypeParameterNamingConventions |
| N012 | intellij:PackageNamingConvention, pmd:PackageCase |
| N014 | intellij:ForwardCompatibility, errorprone:NamedLikeContextualKeyword |
| A001 | intellij:UtilityClassWithPublicConstructor, intellij:UtilityClassWithoutPrivateConstructor, errorprone:PrivateConstructorForUtilityClass, pmd:InstantiableUtilityClass, netbeans:Utility class with visible constructor (api), netbeans:Utility class without constructor (api) |
| A002 | intellij:ClassWithOnlyPrivateConstructors, intellij:NonFinalUtilityClass, pmd:ClassWithOnlyPrivateConstructorsShouldBeFinal, baseline:FinalClass |
| A003 | intellij:PackageVisibleField, intellij:ProtectedField, intellij:PublicField, pmd:MutableStaticState, netbeans:Package Field (encapsulation), netbeans:Protected Field (encapsulation), netbeans:Public Field (encapsulation) |
| A004 | intellij:NonFinalFieldOfException |
| A005 | errorprone:InterfaceWithOnlyStatics, pmd:ConstantsInInterface |
| A006 | intellij:MultipleTopLevelClassesInFile, errorprone:MultipleTopLevelClasses, netbeans:Multiple top-level classes in file (class_structure) |
| A007 | intellij:WrongPackageStatement, errorprone:DefaultPackage, errorprone:PackageLocation, pmd:NoPackage |
| A008 | errorprone:UngroupedOverloads |
| A009 (verworfen) | pmd:FieldDeclarationsShouldBeAtStartOfClass |
| A010 | intellij:BadExceptionDeclared, pmd:SignatureDeclareThrowsException, baseline:ThrowSpecificity |
| A011 | intellij:AbstractClassWithoutAbstractMethods, pmd:AbstractClassWithoutAbstractMethod, pmd:AbstractClassWithoutAnyMethod |
| A012 | intellij:InnerClassMayBeStatic, errorprone:ClassCanBeStatic |
| A014 | pmd:PrematureDeclaration, pmd:VariableDeclarationUsageDistance |
| A015 | intellij:MagicNumber, pmd:AvoidLiteralsInIfCondition |
| A016 (verworfen) | pmd:AvoidDuplicateLiterals |
| A017 | intellij:ClassNameDiffersFromFileName, errorprone:ClassName, pmd:TypeNameMismatch |
| C001 | intellij:CyclomaticComplexity, pmd:CyclomaticComplexity |
| C002 (verworfen) | pmd:NPathComplexity |
| C003 | intellij:OverlyComplexBooleanExpression |
| C004 | pmd:NcssCount |
| C005 | intellij:NonCommentSourceStatements |
| C008 | intellij:OverlyLongLambda |
| C010 | intellij:ParametersPerConstructor, intellij:ParametersPerMethod, errorprone:TooManyParameters, pmd:ExcessiveParameterList, baseline:TooManyArguments |
| C012 (verworfen) | intellij:MethodCount, pmd:TooManyMethods |
| C014 | pmd:AvoidDeeplyNestedIfStmts |
| C016 | intellij:NestedTryStatement |
| C017 (verworfen) | intellij:MultipleReturnPointsPerMethod |
| C018 (verworfen) | intellij:ClassCoupling, pmd:CouplingBetweenObjects |
| C021 | intellij:ThrownExceptionsPerMethod |
| C022 | pmd:CognitiveComplexity |
| D | intellij:DanglingJavadoc, intellij:HtmlTagCanBeJavadocTag, intellij:JavaDoc, intellij:JavadocDeclaration, intellij:MissingJavadoc, intellij:ReplaceWithJavadoc, errorprone:AlmostJavadoc, errorprone:EmptyBlockTag, errorprone:InvalidBlockTag, errorprone:InvalidInlineTag, errorprone:InvalidParam, errorprone:InvalidThrows, errorprone:MalformedInlineTag, errorprone:MissingJavadoc, errorprone:MissingSummary, errorprone:NotJavadoc, errorprone:ReturnFromVoid, errorprone:UnrecognisedJavadocTag, pmd:CommentRequired, pmd:DanglingJavadoc, javac:dangling-doc-comments, eclipse:COMPILER_PB_INVALID_JAVADOC, eclipse:COMPILER_PB_INVALID_JAVADOC_TAGS, eclipse:COMPILER_PB_MISSING_JAVADOC_COMMENTS, eclipse:COMPILER_PB_MISSING_JAVADOC_TAGS, eclipse:COMPILER_PB_MISSING_JAVADOC_TAG_DESCRIPTION |
| X | intellij:AssertStatement, intellij:UseOfProcessBuilder, pmd:CommentContent |

## 5. Eclipse-Clean-Ups und Save-Actions: Zuordnung

Alle Einträge aus `CleanUpConstants` (eclipse.jdt.ui, `master`), gruppiert wie im Clean-Up-Dialog. Die Save-Actions sind eine Teilmenge davon. **fmt** = Formatter, ein Code = bestehende jtk-Regel oder Regel aus Abschnitt 3, **neu** = durch diesen Abgleich ergänzte Regel, **Typen** = ohne Typinformationen nicht umsetzbar. Abschnitt 6 in regel-analyse.md enthält die ältere, kürzere Zuordnung der Save-Actions.

**Code-Organisation und Formatierung**

| Clean-Up | jtk | Hinweis |
|---|---|---|
| `FORMAT_SOURCE_CODE`<br>`FORMAT_CORRECT_INDENTATION` | fmt |  |
| `FORMAT_SOURCE_CODE_CHANGES_ONLY` | fmt | `--lines`/`--character-ranges` |
| `FORMAT_REMOVE_TRAILING_WHITESPACES`<br>`FORMAT_REMOVE_TRAILING_WHITESPACES_ALL`<br>`FORMAT_REMOVE_TRAILING_WHITESPACES_IGNORE_EMPTY` | fmt |  |
| `FORMAT_COMMENT`<br>`FORMAT_JAVADOC` | fmt | `--format-javadoc` |
| `FORMAT_SINGLE_LINE_COMMENT`<br>`FORMAT_MULTI_LINE_COMMENT` | nicht übernommen | Palantir formatiert nur Javadoc |
| `ORGANIZE_IMPORTS`<br>`REMOVE_UNUSED_CODE_IMPORTS` | fmt | fehlende Imports ergänzen geht ohne Classpath nicht |
| `SORT_MEMBERS`<br>`SORT_MEMBERS_ALL` | nicht übernommen | A009 verworfen |

**Code-Stil**

| Clean-Up | jtk | Hinweis |
|---|---|---|
| `CONTROL_STATEMENTS_USE_BLOCKS`<br>`CONTROL_STATEMENTS_USE_BLOCKS_ALWAYS` | S001 |  |
| `CONTROL_STATEMENTS_USE_BLOCKS_NEVER`<br>`CONTROL_STATEMENTS_USE_BLOCKS_NO_FOR_RETURN_AND_THROW` | nicht übernommen | Gegenrichtung zu S001 |
| `EXPRESSIONS_USE_PARENTHESES`<br>`EXPRESSIONS_USE_PARENTHESES_NEVER` | S006 |  |
| `EXPRESSIONS_USE_PARENTHESES_ALWAYS` | B030 (teilweise) | B030 klammert nur gemischte `&&`/`\|\|` |
| `VARIABLE_DECLARATIONS_USE_FINAL`<br>`VARIABLE_DECLARATIONS_USE_FINAL_PARAMETERS` | M011 |  |
| `VARIABLE_DECLARATIONS_USE_FINAL_PRIVATE_FIELDS` | M012 |  |
| `VARIABLE_DECLARATIONS_USE_FINAL_LOCAL_VARIABLES` | M015 |  |
| `NUMBER_SUFFIX` | S015 |  |
| `ELSE_IF` | S053 |  |
| `REDUCE_INDENTATION` | S052 | nur der Fall „else nach Sprung“; das Umdrehen von `if (c) {…} else { return; }` fehlt |
| `ARRAY_WITH_CURLY` | S137 |  |
| `MEMBER_ACCESSES_NON_STATIC_FIELD_USE_THIS`<br>`MEMBER_ACCESSES_NON_STATIC_FIELD_USE_THIS_ALWAYS`<br>`MEMBER_ACCESSES_NON_STATIC_FIELD_USE_THIS_IF_NECESSARY`<br>`MEMBER_ACCESSES_NON_STATIC_METHOD_USE_THIS`<br>`MEMBER_ACCESSES_NON_STATIC_METHOD_USE_THIS_ALWAYS`<br>`MEMBER_ACCESSES_NON_STATIC_METHOD_USE_THIS_IF_NECESSARY` | nicht übernommen | S022 verworfen |
| `MEMBER_ACCESSES_STATIC_QUALIFY_WITH_DECLARING_CLASS`<br>`MEMBER_ACCESSES_STATIC_QUALIFY_WITH_DECLARING_CLASS_FIELD`<br>`MEMBER_ACCESSES_STATIC_QUALIFY_WITH_DECLARING_CLASS_METHOD` | nicht übernommen | Gegenrichtung zu S048 (eigene Klasse als Qualifier überflüssig) |
| `MEMBER_ACCESSES_STATIC_QUALIFY_WITH_DECLARING_CLASS_INSTANCE_ACCESS` | S047 |  |
| `MEMBER_ACCESSES_STATIC_QUALIFY_WITH_DECLARING_CLASS_SUBTYPE_ACCESS` | Typen | braucht die Klassenhierarchie |

**Bedingungen und boolesche Ausdrücke**

| Clean-Up | jtk | Hinweis |
|---|---|---|
| `SIMPLIFY_BOOLEAN_IF_ELSE` | S002 |  |
| `BOOLEAN_VALUE_RATHER_THAN_COMPARISON` | S003 |  |
| `RAISE_EMBEDDED_IF` | S004 |  |
| `DOUBLE_NEGATION`<br>`PUSH_DOWN_NEGATION` | S070, S055 |  |
| `STRICTLY_EQUAL_OR_DIFFERENT` | S071 |  |
| `TERNARY_OPERATOR` | S072 |  |
| `OPERAND_FACTORIZATION` | S073 |  |
| `USE_LAZY_LOGICAL_OPERATOR` | B092 |  |
| `UNREACHABLE_BLOCK` | B197 |  |
| `MERGE_CONDITIONAL_BLOCKS` | B198, S184 | B198: ganze Zweige identisch; S184: aufeinanderfolgende if mit Sprung |
| `REDUNDANT_IF_CONDITION` | S180 | neu |
| `CONTROLFLOW_MERGE` | S181 | neu |
| `PULL_OUT_IF_FROM_IF_ELSE` | S182 | neu |
| `REDUNDANT_FALLING_THROUGH_BLOCK_END` | S183 | neu |
| `ONE_IF_RATHER_THAN_DUPLICATE_BLOCKS_THAT_FALL_THROUGH` | S184 | neu |
| `REMOVE_REDUNDANT_COMPARISON_STATEMENT` | S185 | neu |
| `CHECK_SIGN_OF_BITWISE_OPERATION` | B284 | neu |
| `INVERT_EQUALS` | B008 |  |
| `EVALUATE_NULLABLE` | S084 |  |
| `INSTANCEOF` | S187 | neu |

**Schleifen und switch**

| Clean-Up | jtk | Hinweis |
|---|---|---|
| `CONTROL_STATEMENTS_CONVERT_FOR_LOOP_TO_ENHANCED`<br>`CONTROL_STATEMENTS_CONVERT_FOR_LOOP_ONLY_IF_LOOP_VAR_USED` | M007, M022 | M007 Index-Schleife, M022 Iterator-Schleife |
| `USE_SWITCH`<br>`USE_SWITCH_FOR_INSTANCEOF_PATTERN` | M021 |  |
| `CONTROL_STATEMENTS_CONVERT_TO_SWITCH_EXPRESSIONS` | M002 |  |
| `REMOVE_USELESS_CONTINUE` | S057 |  |
| `BREAK_LOOP` | P035 |  |
| `UNLOOPED_WHILE` | B202 |  |
| `DO_WHILE_RATHER_THAN_WHILE` | S062 (teilweise) | Eclipse erkennt auch eine vorher sicher wahre Bedingung |
| `CONTROL_STATEMENTS_USE_ADD_ALL` | S140 |  |
| `ARRAYS_FILL` | S141 |  |
| `JOIN` | M060 | neu |

**Variablen, Zuweisungen, return**

| Clean-Up | jtk | Hinweis |
|---|---|---|
| `RETURN_EXPRESSION` | S005, S044 |  |
| `REMOVE_USELESS_RETURN` | S010 |  |
| `OVERRIDDEN_ASSIGNMENT` | B026 |  |
| `OVERRIDDEN_ASSIGNMENT_MOVE_DECL` | S043 |  |
| `SINGLE_USED_FIELD` | A050 |  |
| `PULL_UP_ASSIGNMENT` | B012 | B012 meldet, hat aber noch keinen Fix: Zuweisung vor das if ziehen |
| `EXTRACT_INCREMENT` | B118 | B118 meldet, hat aber noch keinen Fix: `++`/`--` als eigene Anweisung herausziehen |
| `USE_VAR` | M006 |  |

**Unnötiger Code**

| Clean-Up | jtk | Hinweis |
|---|---|---|
| `REMOVE_REDUNDANT_SEMICOLONS` | S007 |  |
| `REMOVE_REDUNDANT_MODIFIERS` | S008 |  |
| `REDUNDANT_SUPER_CALL` | S011 |  |
| `REMOVE_UNUSED_CODE_LOCAL_VARIABLES` | B026 |  |
| `REMOVE_UNUSED_CODE_PRIVATE_MEMBERS`<br>`REMOVE_UNUSED_CODE_PRIVATE_FELDS`<br>`REMOVE_UNUSED_CODE_PRIVATE_METHODS` | B027 |  |
| `REMOVE_UNUSED_CODE_PRIVATE_TYPES` | B027 (erweitern) | B027 prüft noch keine privaten Typen |
| `REMOVE_UNUSED_CODE_PRIVATE_CONSTRUCTORS` | nicht übernommen | private Konstruktoren verhindern meist bewusst die Instanziierung (A001) |
| `REMOVE_UNUSED_CODE_METHOD_PARAMETERS` | B028 |  |
| `REMOVE_UNNECESSARY_CASTS` | S101 (teilweise), Typen | ohne Typen nur Casts von Literalen und lokal bekannten Typen |
| `REMOVE_UNNECESSARY_ARRAY_CREATION` | S136 |  |
| `REDUNDANT_COMPARATOR` | S157 |  |
| `STATIC_INNER_CLASS` | A012 |  |
| `REMOVE_REDUNDANT_TYPE_ARGUMENTS` | M004 |  |
| `INSERT_INFERRED_TYPE_ARGUMENTS` | nicht übernommen | Gegenrichtung zu M004 |

**Java-Features und APIs**

| Clean-Up | jtk | Hinweis |
|---|---|---|
| `USE_PATTERN_MATCHING_FOR_INSTANCEOF` | M001 |  |
| `CONVERT_FUNCTIONAL_INTERFACES`<br>`USE_LAMBDA` | M017 |  |
| `USE_ANONYMOUS_CLASS_CREATION` | nicht übernommen | Gegenrichtung zu M017 |
| `ALSO_SIMPLIFY_LAMBDA`<br>`SIMPLIFY_LAMBDA_EXPRESSION_AND_METHOD_REF` | S021, S130 |  |
| `COMPARING_ON_CRITERIA` | M032 |  |
| `TRY_WITH_RESOURCE` | M023 |  |
| `MULTI_CATCH` | M010 |  |
| `STRINGCONCAT_TO_TEXTBLOCK` | M009 |  |
| `USE_MODULE_IMPORTS` | M061 | neu |
| `USE_STRING_IS_BLANK`<br>`SUBSTRING` | S110 |  |
| `PLAIN_REPLACEMENT` | P009 |  |
| `PRECOMPILE_REGEX` | P010 |  |
| `STRINGBUFFER_TO_STRINGBUILDER`<br>`STRINGBUFFER_TO_STRINGBUILDER_FOR_LOCALS` | P049 |  |
| `STRINGBUILDER` | nicht übernommen | Gegenrichtung zu S118; javac optimiert Verkettung selbst |
| `NO_STRING_CREATION`<br>`VALUEOF_RATHER_THAN_INSTANTIATION` | S019 |  |
| `PRIMITIVE_PARSING`<br>`PRIMITIVE_SERIALIZATION` | S124 |  |
| `PRIMITIVE_COMPARISON` | S186 | neu |
| `PRIMITIVE_RATHER_THAN_WRAPPER` | P012 |  |
| `USE_AUTOBOXING`<br>`USE_UNBOXING` | S103 |  |
| `PREFER_BOOLEAN_LITERAL` | S081 |  |
| `USE_DIRECTLY_MAP_METHOD` | S144 |  |
| `COLLECTION_CLONING`<br>`MAP_CLONING` | S139 |  |
| `CONSTANTS_FOR_SYSTEM_PROPERTY`<br>`CONSTANTS_FOR_SYSTEM_PROPERTY_FILE_SEPARATOR`<br>`CONSTANTS_FOR_SYSTEM_PROPERTY_FILE_ENCODING`<br>`CONSTANTS_FOR_SYSTEM_PROPERTY_PATH_SEPARATOR`<br>`CONSTANTS_FOR_SYSTEM_PROPERTY_LINE_SEPARATOR`<br>`CONSTANTS_FOR_SYSTEM_PROPERTY_BOXED`<br>`CONSTANTS_FOR_SYSTEM_PROPERTY_JAVA_RUNTIME_VERSION`<br>`CONSTANTS_FOR_SYSTEM_PROPERTY_JAVA_SPECIFICATION_VERSION` | S125 |  |
| `STANDARD_COMPARISON` | B054 |  |
| `USE_OBJECTS_EQUALS` | S085 |  |
| `MODERNIZE_HASH` | nicht übernommen | `Objects.hash` boxt jeden Wert; widerspricht P038 |
| `REPLACE_DEPRECATED_CALLS`<br>`REPLACE_DEPRECATED_FIELDS` | M050 (teilweise), Typen | ohne Typen nur die bekannte Liste von JDK-APIs |
| `VARIABLE_DECLARATION_USE_TYPE_ARGUMENTS_FOR_RAW_TYPE_REFERENCES` | B236, Typen | B236 meldet; die Typargumente zu ergänzen braucht Typen |

**Fehlende Code-Teile**

| Clean-Up | jtk | Hinweis |
|---|---|---|
| `ADD_MISSING_ANNOTATIONS`<br>`ADD_MISSING_ANNOTATIONS_OVERRIDE`<br>`ADD_MISSING_ANNOTATIONS_OVERRIDE_FOR_INTERFACE_METHOD_IMPLEMENTATION` | A062 (teilweise), Typen | ohne Typen nur equals/hashCode/toString und Obertypen derselben Datei |
| `ADD_MISSING_ANNOTATIONS_DEPRECATED` | M013 |  |
| `ADD_MISSING_SERIAL_VERSION_ID`<br>`ADD_MISSING_SERIAL_VERSION_ID_DEFAULT` | B240 | Fix mit `1L` möglich, aber unsicher (ändert die Serialisierungs-ID) |
| `ADD_MISSING_SERIAL_VERSION_ID_GENERATED` | Typen | braucht den Compiler |
| `ADD_MISSING_METHODES` | nicht übernommen | Compilerfehler, IDE-Funktion |
| `ADD_NECESSARY_SUPPRESS_WARNINGS`<br>`REMOVE_UNNECESSARY_SUPPRESS_WARNINGS` | Typen | braucht die Compilerwarnungen |
| `ADD_MISSING_NLS_TAGS`<br>`REMOVE_UNNECESSARY_NLS_TAGS` | nicht übernommen | Eclipse-spezifisch |

**Profil-Einstellungen (keine Clean-Ups)**

| Clean-Up | jtk | Hinweis |
|---|---|---|
| `SHOW_CLEAN_UP_WIZARD`<br>`CLEANUP_PROFILES`<br>`CLEANUP_ON_SAVE_PROFILE` | – |  |

## 6. Nicht übernommen

| Grund | Bedeutung | Anzahl |
|---|---|---:|
| TYP | braucht Typen beliebiger Ausdrücke, Overload-Auflösung oder die Klassenhierarchie außerhalb der Datei | 203 |
| FLUSS | braucht Datenfluss-, Nullness- oder interprozedurale Analyse | 47 |
| LIB | betrifft eine Bibliothek außerhalb von JDK, JUnit, TestNG, AssertJ, Hamcrest, Mockito, SLF4J, Log4j 2, JUL und Guava | 260 |
| IDE | Editor-Funktion, Refactoring ohne Befund oder Projekt-, Build-, Modul- und Classfile-Ebene | 140 |
| GESCHMACK | Gegenregel oder Stilmeinung, die mit dem Palantir-Stil oder bestehenden jtk-Regeln kollidiert | 117 |
| ALT | für Java 17+ gegenstandslos (JUnit 3, alte APIs) oder ein Fehler, den javac ohnehin meldet | 49 |
| FMT | Layout oder Imports, die der Formatter übernimmt | 15 |
| INTERN | toolinterne Regel (Error-Prone-API, Plugin-Entwicklung, javac-Optionen) | 30 |

<details><summary>TYP (203)</summary>

- **IntelliJ IDEA Community:** AbstractClassExtendsConcreteClass, AbstractClassNeverImplemented, AbstractClassWithOnlyOneDirectInheritor, AbstractMethodOverridesAbstractMethod, AbstractMethodOverridesConcreteMethod, AmbiguousFieldAccess, AmbiguousMethodCall, AssertEqualsMayBeAssertSame, AssignmentToSuperclassField, AutoBoxing, AutoUnboxing, BoundedWildcard, CastCanBeRemovedNarrowingVariableType, CastToIncompatibleInterface, ClassInheritanceDepth, Deprecation, EmptyMethod, ExpectedExceptionNeverThrown, FeatureEnvy, FunctionalExpressionCanBeFolded, ImplicitNumericConversion, InnerClassReferencedViaSubclass, InstanceofIncompatibleInterface, InterfaceWithOnlyOneDirectInheritor, JavaLangInvokeHandleSignature, JavaReflectionMemberAccess, JavadocReference, Junit4RunWithInspection, MagicConstant, MarkedForRemoval, MeaninglessRecordAnnotationInspection, MethodOverloadsParentMethod, MethodOverridesInaccessibleMethodOfSuper, MethodOverridesStaticMethod, MethodReturnAlwaysConstant, NonSerializableFieldInSerializableClass, NonSerializableObjectPassedToObjectStream, NonSynchronizedMethodOverridesSynchronizedMethod, NullableProblems, ObjectEquality, ObjectEqualsCanBeEquality, ObjectToString, OverlyStrongTypeCast, OverriddenMethodCallDuringObjectConstruction, ParameterNameDiffersFromOverriddenParameter, ParameterTypePreventsOverriding, PreviewFeature, ProblematicVarargsMethodOverride, RedundantImplements, RedundantLambdaParameterType, RedundantThrows, RedundantTypeArguments, RefusedBequest, SerializableDeserializableClassInSecureContext, SerializableStoresNonSerializable, SerializableWithUnconstructableAncestor, SortedCollectionWithNonComparableKeys, StaticCallOnSubclass, StaticFieldReferenceOnSubclass, StaticInheritance, SuspiciousTernaryOperatorInVarargsCall, TestOnlyProblems, TooBroadCatch, TooBroadThrows, TypeMayBeWeakened, UnreachableCatch, UnstableApiUsage, UnstableTypeUsedInSignature, UseOfConcreteClass
- **Error Prone:** AmbiguousMethodReference, ArrayFillIncompatibleType, AssertSameIncompatible, AssertThrowsMinimizer, BoxingComparator, CatchingUnchecked, CheckReturnValue, CheckedExceptionNotThrown, CollectionIncompatibleType, CollectionToArraySafeParameter, ComputeIfAbsentAmbiguousReference, DoNotCall, FunctionalInterfaceMethodChanged, FutureReturnValueIgnored, GetClassOnAnnotation, ImpossibleNullComparison, InexactVarargsConditional, IntFloatConversion, InterruptedExceptionSwallowed, IsInstanceIncompatibleType, JUnitIncompatibleType, LongDoubleConversion, LongFloatConversion, MockIllegalThrows, NestedInstanceOfConditions, NonCanonicalStaticImport, NonCanonicalStaticMemberImport, NonCanonicalType, ObjectToString, Overrides, PreferJavaTimeOverload, RecordComponentAccessorAnnotationConflict, RedundantThrows, ReferenceEquality, UnnecessaryLongToIntConversion, UnnecessaryMethodReference, UnsafeWildcard
- **PMD 7:** ConfusingArgumentToVarargsMethod, GodClass, InternalApiUsage, LawOfDemeter, NonSerializableClass
- **Apache NetBeans java.hints:** .equals on Incompatible Types (bugs), @CheckReturnValue (bugs), Annotations without runtime Retention (bugs), DN_TooStrongCast (abstraction), DN_assertEqualsIncovertibleTypes (testing), Incompatible cast/instanceof (bugs), Suspicious Collections.toArray() call (bugs), Suspicious method call (bugs), Throwable method result is ignored (bugs), Too broad 'catch' clause (rules15), Use specific catch (rules15)
- **Picnic Error Prone Support:** AssertJDurationRules (Rest), AssertJIterableRules (Rest), AssertJMapRules (Rest), AssertJOptionalRules (Rest), AssertJPathRules (Rest), AssertJPrimitiveRules (Rest), AssertJRules, AssertJStreamRules (Rest), AssortedRules (Rest), BigDecimalRules (Rest), CollectionRules (Rest), ComparatorRules (Rest), DequeRules, DoubleStreamRules (Rest), EqualityRules (Rest), ExplicitArgumentEnumeration, FileRules (Rest), ImmutableEnumSetRules, ImmutableListMultimapRules (Rest), ImmutableMultisetRules (Rest), ImmutableSetMultimapRules (Rest), ImmutableSortedMapRules (Rest), ImmutableSortedMultisetRules (Rest), ImmutableSortedSetRules (Rest), ImmutableTableRules (Rest), IntStreamRules (Rest), JUnitToAssertJRules, LongStreamRules (Rest), MapRules (Rest), MockitoRules (Rest), MultimapRules, OptionalRules (Rest), PrimitiveComparison, RandomGeneratorRules, StreamRules (Rest), StringRules (Rest), TestNGToAssertJRules, TimeRules (Rest)
- **Palantir baseline-error-prone:** CatchSpecificity, CompileTimeConstantViolatesLiskovSubstitution, ConsistentOverrides, DangerousIdentityKey, JUnit5SuiteMisuse, NonComparableStreamSort, PreferCollectionTransform, PreferListsPartition, RedundantMethodReference, StrictCollectionIncompatibleType, deprecation, removal
- **javac `-Xlint`:** auxiliaryclass, compiler.warn.method.redundant.typeargs, compiler.warn.unreachable.catch, deprecation, overloads, overrides, removal, unchecked, varargs
- **Eclipse JDT Compiler:** COMPILER_PB_AUTOBOXING, COMPILER_PB_DEPRECATION, COMPILER_PB_DEPRECATION_IN_DEPRECATED_CODE, COMPILER_PB_DEPRECATION_WHEN_OVERRIDING_DEPRECATED_METHOD, COMPILER_PB_FIELD_HIDING, COMPILER_PB_HIDDEN_CATCH_BLOCK, COMPILER_PB_INCOMPATIBLE_NON_INHERITED_INTERFACE_METHOD, COMPILER_PB_INDIRECT_STATIC_ACCESS, COMPILER_PB_INVALID_JAVADOC_TAGS__DEPRECATED_REF, COMPILER_PB_INVALID_JAVADOC_TAGS__NOT_VISIBLE_REF, COMPILER_PB_MISSING_SYNCHRONIZED_ON_INHERITED_METHOD, COMPILER_PB_OVERRIDING_METHOD_WITHOUT_SUPER_INVOCATION, COMPILER_PB_OVERRIDING_PACKAGE_DEFAULT_METHOD, COMPILER_PB_POTENTIALLY_MISSING_STATIC_ON_METHOD, COMPILER_PB_TERMINAL_DEPRECATION, COMPILER_PB_UNCHECKED_TYPE_OPERATION, COMPILER_PB_UNUSED_DECLARED_THROWN_EXCEPTION, COMPILER_PB_UNUSED_DECLARED_THROWN_EXCEPTION_WHEN_OVERRIDING, COMPILER_PB_UNUSED_PARAMETER_WHEN_IMPLEMENTING_ABSTRACT, COMPILER_PB_UNUSED_PARAMETER_WHEN_OVERRIDING_CONCRETE, COMPILER_PB_UNUSED_TYPE_ARGUMENTS_FOR_METHOD_INVOCATION, COMPILER_PB_VARARGS_ARGUMENT_NEED_CAST

</details>

<details><summary>FLUSS (47)</summary>

- **IntelliJ IDEA Community:** ChannelResource, ConstantValue, DataFlowIssue, IOResource, IdempotentLoopBody, InstanceVariableInitialization, InstanceVariableUninitializedUse, JDBCResource, JNDIResource, NonStrictComparisonCanBeEquality, NotNullFieldNotInitialized, NotNullParameterReceivesNull, OptionalGetWithoutIsPresent, ReadObjectInitialization, RedundantOperationOnEmptyContainer, SocketResource, SourceToSinkFlow, StaticVariableInitialization, StaticVariableUninitializedUse, UnreachableCode, WriteOnlyObject
- **Error Prone:** ByteBufferBackingArray, EqualsBrokenForNull, GuardedBy, MockNotUsedInProduction, UnsafeFinalization
- **PMD 7:** CloseResource, InsufficientStringBufferDeclaration
- **Apache NetBeans java.hints:** Dead Branch (bugs), Null Pointer Dereference (bugs)
- **Eclipse JDT Compiler:** COMPILER_PB_ANNOTATED_TYPE_ARGUMENT_TO_UNANNOTATED, COMPILER_PB_DEAD_CODE, COMPILER_PB_INCLUDE_ASSERTS_IN_NULL_ANALYSIS, COMPILER_PB_NONNULL_PARAMETER_ANNOTATION_DROPPED, COMPILER_PB_NONNULL_TYPEVAR_FROM_LEGACY_INVOCATION, COMPILER_PB_NULL_ANNOTATION_INFERENCE_CONFLICT, COMPILER_PB_NULL_REFERENCE, COMPILER_PB_NULL_SPECIFICATION_VIOLATION, COMPILER_PB_NULL_UNCHECKED_CONVERSION, COMPILER_PB_PESSIMISTIC_NULL_ANALYSIS_FOR_FREE_TYPE_VARIABLES, COMPILER_PB_POTENTIALLY_UNCLOSED_CLOSEABLE, COMPILER_PB_POTENTIAL_NULL_REFERENCE, COMPILER_PB_RECOMMENDED_RESOURCE_MANAGEMENT, COMPILER_PB_REDUNDANT_NULL_ANNOTATION, COMPILER_PB_REDUNDANT_NULL_CHECK, COMPILER_PB_SYNTACTIC_NULL_ANALYSIS_FOR_FIELDS, COMPILER_PB_UNCLOSED_CLOSEABLE

</details>

<details><summary>LIB (260)</summary>

- **IntelliJ IDEA Community:** BlockingMethodInNonBlockingContext, CheckJsfComponentUnderViewTag, Contract, DomModelInspection, FieldAccessNotGuarded, HibernateResource, ImplicitSubclassInspection, InjectionNotApplicable, InstanceGuardedByStatic, LanguageMismatch, MissingDeprecatedAnnotationOnScheduledForRemovalApi, MustAlreadyBeRemovedApi, NonExtendableApiUsage, NonFinalFieldInImmutable, NonFinalGuard, NonSerializableObjectBoundToHttpSession, OverrideOnly, PatternNotApplicable, PatternOverriddenByNonAnnotatedMethod, PatternValidation, RedundantScheduledForRemovalAnnotation, StaticGuardedByInstance, UnknownGuard, UnknownLanguage, UnsatisfiedRange, UsagesOfObsoleteApi, UseOfJDBCDriverClass
- **Error Prone:** AddNullMarkedToClass, AddNullMarkedToPackageInfo, AlmostExhaustiveTestParameter, AndroidInjectionBeforeSuper, AnnotateFormatMethod, AnnotationMirrorToString, AnnotationValueToString, AssistedInjectAndInjectOnConstructors, AssistedInjectAndInjectOnSameConstructor, AutoFactoryAtInject, AutoValueBoxedValues, AutoValueBuilderDefaultsInConstructor, AutoValueConstructorOrderChecker, AutoValueFinalMethods, AutoValueImmutableFields, AutoValueSubclassLeaked, AvoidValueSetter, BinderIdentityRestoredDangerously, BindingToUnqualifiedCommonType, BundleDeserializationCast, CanIgnoreReturnValueSuggester, ChainedAssertionLosesContext, CloseableProvides, CompatibleWithAnnotationMisuse, CompileTimeConstant, DaggerProvidesNull, DoNotCallSuggester, DoNotMock, DoNotMockAutoValue, EmptySetMultibindingContributions, EqualsMissingNullable, ExtendsAutoValue, FieldMissingNullable, FloggerArgumentToString, FloggerFormatString, FloggerLogString, FloggerLogVarargs, FloggerLogWithCause, FloggerMessageFormat, FloggerPerWithoutRateLimit, FloggerRedundantIsEnabled, FloggerRequiredModifiers, FloggerSplitLogStatement, FloggerStringConcatenation, FloggerWithCause, FloggerWithoutCause, ForOverride, FormatStringAnnotation, FragmentInjection, FragmentNotInstantiable, GuiceAssistedInjectScoping, GuiceAssistedParameters, GuiceInjectOnFinalField, GuiceNestedCombine, HardCodedSdCardPath, IgnoredPureGetter, Immutable, ImmutableRefactoring, ImplementAssertionWithChaining, IncompatibleArgumentType, IncompatibleModifiers, InjectInvalidTargetingOnScopingAnnotation, InjectMoreThanOneScopeAnnotationOnClass, InjectOnConstructorOfAbstractClass, InjectOnMemberAndConstructor, InjectScopeAnnotationOnInterfaceOrAbstractClass, InjectedConstructorAnnotations, InlineMeInliner, InlineMeSuggester, InlineMeValidator, IsLoggableTagLength, JUnitParameterMethodNotFound, JavaxInjectOnAbstractMethod, JavaxInjectOnFinalField, JodaConstructors, JodaDateTimeConstants, JodaDurationWithMillis, JodaInstantWithMillis, JodaNewPeriod, JodaPlusMinusLong, JodaTimeConverterManager, JodaToSelf, JodaWithDurationAddedLong, LiteByteStringUtf8, LiteEnumValueOf, LiteProtoToString, LoopToTestParameter, MislabeledAndroidString, MisplacedScopeAnnotations, MissingRuntimeRetention, MissingTestCall, MixedDescriptors, MoreThanOneInjectableConstructor, MoreThanOneQualifier, MultipleNullnessAnnotations, MustBeClosedChecker, MutableGuiceModule, NoAllocation, NoCanIgnoreReturnValueOnClasses, NonFinalCompileTimeConstant, NullArgumentForNonNullParameter, NullableConstructor, NullableOnContainingClass, NullableOptional, NullablePrimitive, NullablePrimitiveArray, NullableTypeParameter, NullableVoid, NullableWildcard, OutlineNone, OverlappingQualifierAndScopeAnnotation, OverridesGuiceInjectableMethod, OverridesJavaxInjectableMethod, ParameterMissingNullable, ParcelableCreator, PrivateConstructorForNoninstantiableModule, PrivateSecurityContractProtoAccess, ProtoBuilderReturnValueIgnored, ProtoDurationGetSecondsGetNano, ProtoStringFieldReferenceEquality, ProtoTimestampGetSecondsGetNano, ProtoTruthMixedDescriptors, ProtocolBufferOrdinal, ProvidesMethodOutsideOfModule, QualifierOrScopeOnInjectMethod, QualifierWithTypeUse, RectIntersectReturnValueIgnored, RefersToDaggerCodegen, RequiredModifiers, RestrictedApi, ReturnMissingNullable, RobolectricShadowDirectlyOn, RxReturnValueIgnored, ScopeOnModule, SetUnrecognized, ShouldHaveEvenArgs, SimplifyTestParameter, SingleTestParameter, SingleValueTestParameter, StaticOrDefaultInterfaceMethod, StronglyTypeByteString, SwigMemoryLeak, TestParametersNotInitialized, ThreadSafe, TruthAssertExpected, TruthConstantAsserts, TruthContainsExactlyElementsInUsage, TruthGetOrDefault, TruthIncompatibleType, UnnecessarilyUsedValue, UnnecessarilyVisible, UnnecessaryQualifier, UnnecessarySetDefault, UseBinds, UsingJsr305CheckReturnValue, Var, VoidMissingNullable, WakelockReleasedDangerously, WrongOneof
- **PMD 7:** CallSuperFirst, CallSuperLast, DoNotHardCodeSDCard, LocalHomeNamingConvention, LocalInterfaceSessionNamingConvention, MDBAndSessionBeanNamingConvention, RemoteInterfaceNamingConvention, RemoteSessionInterfaceNamingConvention, StaticEJBFieldShouldBeFinal, UseCorrectExceptionLogging, UseIOStreamsWithApacheCommonsFileItem, UseProperClassLoader
- **Picnic Error Prone Support:** AmbiguousJsonCreator, AutowiredConstructor, EmptyMonoZip, FluxFlatMapUsage, FluxImplicitBlock, ImmutablesSortedSetComparator, Jackson2Rules, Jackson3Rules, MicrometerRules, MongoDBTextFilterUsage, NestedPublishers, NonEmptyMono, ReactorRules, RequestMappingAnnotation, RequestParamType, RxJava2AdapterRules, SpringMvcAnnotation, SpringTestRules, WebClientRules
- **Palantir baseline-error-prone:** AfterburnerJavaIncompatibility, ArgContainingArgs, AssertNoArgs, AutoCloseableMustBeClosed, ConjureEndpointDeprecatedForRemoval, DangerousCollapseKeysUsage, DangerousImmutablesToStringDoNotLog, DangerousJsonTypeInfoUsage, DangerousRecordToStringDoNotLog, DangerousThrowableMessageSafeArg, DangerousToStringDoNotLog, ForbidJavaxParameterType, GradleCacheableArgumentProvider, GradleCacheableTaskAction, GradleProviderToString, IllegalSafeLoggingArgument, ImmutablesBuilderMissingInitialization, ImmutablesReferenceEquality, ImmutablesStyle, IncubatingMethod, JooqBatchWithoutBindArgs, JooqResultStreamLeak, LogSafePreconditionsMessageFormat, LoggingDoNotLog, LogsafeArgName, LogsafeArrayArg, LogsafeThrowableArgument, PreferImmutableStreamExCollections, PreferSafeLoggableExceptions, PreferSafeLogger, PreferSafeLoggingPreconditions, PreventTokenLogging, ResourceIdentifierGetEqualsUsage, SafeLoggingExceptionMessageFormat, SafeLoggingPropagation, Slf4jLogsafeArgs, UnsafeGaugeRegistration, ValidateConstantMessage
- **Modernizer Maven Plugin:** AutoValue → Records, Guice Provider → jakarta.inject.Provider
- **Eclipse JDT Compiler:** COMPILER_PB_INCOMPATIBLE_OWNING_CONTRACT, COMPILER_PB_MISSING_NONNULL_BY_DEFAULT_ANNOTATION, COMPILER_PB_NON_NLS_STRING_LITERAL

</details>

<details><summary>IDE (140)</summary>

- **IntelliJ IDEA Community:** AssertionCanBeIf, BooleanMethodIsAlwaysInverted, ClassIndependentOfModule, ClassOnlyUsedInOneModule, ClassOnlyUsedInOnePackage, ClassOverriddenAtRuntime, ClassUnconnectedToPackage, ClassWithTooManyDependencies, ClassWithTooManyDependents, ClassWithTooManyTransitiveDependencies, ClassWithTooManyTransitiveDependents, ConstantExpression, CyclicClassDependency, CyclicPackageDependency, Dependency, DeprecatedClassUsageInspection, DisjointPackage, ExceptionPackage, ExtractMethodRecommender, FieldCanBeMovedToSubclass, FrequentlyUsedInheritorInspection, HTML, IllegalDependencyOnInternalPackage, InconsistentLanguageLevel, InterfaceNeverImplemented, Java9ModuleExportsPackageToItself, Java9RedundantRequiresStatement, Java9ReflectionClassVisibility, Java9UndeclaredServiceUsage, JavaEmptyModuleInfoFile, JavaModuleDefinition, JavaModuleNaming, JavaRequiresAutoModule, LambdaBodyCanBeCodeBlock, LambdaCanBeReplacedWithAnonymous, LambdaParameterTypeCanBeSpecified, Local_CustomHtmlAttributes, Local_CustomHtmlTags, Local_NotRequiredHtmlAttributes, MethodRefCanBeReplacedWithLambda, MigrateAssertToMatcherAssert, MigrateFromJavaLangIo, MissingFinalNewline, MissingPackageInfo, MisspelledHeader, ModuleWithTooFewClasses, ModuleWithTooManyClasses, MultiCatchCanBeSplit, OptionalToIf, PackageDotHtmlMayBePackageInfo, PackageInMultipleModules, PackageWithTooFewClasses, PackageWithTooManyClasses, PatternVariablesCanBeReplacedWithCast, ReassignedVariable, RecordCanBeClass, RemoveLiteralUnderscores, SerializableCanHaveDefaultSerialUID, Since15, StaticFieldCanBeMovedToUse, StaticImportCanBeUsed, StaticMethodOnlyUsedInOneClass, StreamToLoop, SuspiciousPackagePrivateAccess, SwitchLabeledRuleCanBeCodeBlock, TestFailedLine, TestInProductSource, TextBlockBackwardMigration, TryStatementWithMultipleResources, UnnecessaryModuleDependencyInspection, UnusedLibrary, WeakerAccess, unused
- **Error Prone:** BootclasspathShadowing, ClasspathShadowing, Java8ApiChecker
- **Apache NetBeans java.hints:** ? (?), ? (General), ? (general), Convert Lambda Parameter Type to 'var' Type (suggestions), Convert integer constant to different base (suggestions), Create Subclass (suggestions), DN_AnnoProcessor_ObsoleteSupportedSource (rules15), DN_CanUseVT (suggestions), DN_CheckRegex (general), DN_ConvertTextBlockToString (rules15), DN_FlipOperands (suggestions), DN_ForLoopToFunctionalHint (general), DN_ImplementMethods (suggestions), DN_InvertIf (suggestions), DN_ToOrIf (suggestions), DN_addExplicitLambdaParameters (suggestions), DN_expression2Return (suggestions), DN_lambda2Class (suggestions), DN_memberReference2Lambda (suggestions), DN_org.netbeans.modules.java.hints.suggestions.Move.moveInitialization (suggestions), DN_org.netbeans.modules.java.hints.suggestions.Tiny.extractIf (suggestions), DN_splitIfCondition (suggestions), Expand Enhanced For Loop (suggestions), Fill Missing Cases to Switch (suggestions), Flip .equals (suggestions), Inline Redundant Variable (suggestions), Replace chain of ifs with switch (suggestions), Split Declaration (suggestions), Static imports (rules15)
- **javac `-Xlint`:** classfile, exports, incubating, missing-explicit-ctor, module, opens, output-file-clash, path, preview, requires-automatic, requires-transitive-automatic, restricted
- **Eclipse JDT Compiler:** COMPILER_PB_API_LEAKS, COMPILER_PB_BOOLEAN_METHOD_THROWING_EXCEPTION, COMPILER_PB_DISCOURAGED_REFERENCE, COMPILER_PB_ENABLE_PREVIEW_FEATURES, COMPILER_PB_FORBIDDEN_REFERENCE, COMPILER_PB_INVALID_JAVADOC_TAGS_VISIBILITY, COMPILER_PB_MISSING_JAVADOC_COMMENTS_OVERRIDING, COMPILER_PB_MISSING_JAVADOC_COMMENTS_VISIBILITY, COMPILER_PB_MISSING_JAVADOC_TAGS_METHOD_TYPE_PARAMETERS, COMPILER_PB_MISSING_JAVADOC_TAGS_OVERRIDING, COMPILER_PB_MISSING_JAVADOC_TAGS_VISIBILITY, COMPILER_PB_MISSING_JAVADOC_TAG_DESCRIPTION_ALL_STANDARD_TAGS, COMPILER_PB_MISSING_JAVADOC_TAG_DESCRIPTION_NO_TAG, COMPILER_PB_MISSING_JAVADOC_TAG_DESCRIPTION_RETURN_TAG, COMPILER_PB_MISSING_OVERRIDE_ANNOTATION_FOR_INTERFACE_METHOD_IMPLEMENTATION, COMPILER_PB_REPORT_PREVIEW_FEATURES, COMPILER_PB_SPECIAL_PARAMETER_HIDING_FIELD, COMPILER_PB_UNAVOIDABLE_GENERIC_TYPE_PROBLEMS, COMPILER_PB_UNLIKELY_COLLECTION_METHOD_ARGUMENT_TYPE_STRICT, COMPILER_PB_UNSTABLE_AUTO_MODULE_NAME, COMPILER_PB_UNUSED_DECLARED_THROWN_EXCEPTION_EXEMPT_EXCEPTION_AND_THROWABLE, COMPILER_PB_UNUSED_DECLARED_THROWN_EXCEPTION_INCLUDE_DOC_COMMENT_REFERENCE, COMPILER_PB_UNUSED_PARAMETER_INCLUDE_DOC_COMMENT_REFERENCE

</details>

<details><summary>GESCHMACK (117)</summary>

- **IntelliJ IDEA Community:** AnonymousInnerClass, ArrayCreationWithoutNewKeyword, BreakStatement, CallToSimpleGetterInClass, CallToSimpleSetterInClass, CallToSuspiciousStringMethod, ChainedMethodCall, CharacterComparison, ClassHasNoToStringMethod, ClassWithoutConstructor, ClassWithoutLogger, ClassWithoutNoArgConstructor, CollectionsMustHaveInitialCapacity, ConditionalCanBeOptional, ConditionalExpression, ConstantDeclaredInAbstractClass, ContinueStatement, Convert2streamapi, DiamondCanBeReplacedWithExplicitTypeArguments, EnhancedSwitchBackwardMigration, ExplicitToImplicitClassMigration, FinalClass, FinalMethod, FoldExpressionIntoStream, ForLoopWithMissingComponent, HardcodedFileSeparators, HardcodedLineSeparators, IfCanBeAssertion, ImplicitCallToSuper, ImplicitToExplicitClassBackwardMigration, InnerClassOnInterface, Java8MapForEach, LawOfDemeter, MethodCallInLoopCondition, MethodWithMultipleLoops, NestedMethodCall, NonReproducibleMathCall, NumericToString, ObjectAllocationInLoop, PackageVisibleInnerClass, ProtectedInnerClass, PublicConstructor, PublicInnerClass, PublicMethodNotExposedInInterface, PublicMethodWithoutLogging, ReturnThis, SerializableHasSerializationMethods, SingleCharacterStartsWith, SingleClassImport, SingleElementAnnotation, SingleStatementInBlock, Singleton, StringBufferMustHaveInitialCapacity, StringConcatenation, SwitchStatement, ThrowsRuntimeException, UncheckedExceptionClass, UnnecessaryFinalOnLocalVariableOrParameter, UnnecessaryThis, UnqualifiedFieldAccess, UnqualifiedInnerClassAccess, UnqualifiedMethodAccess, UnqualifiedStaticUsage, UseOfAnotherObjectsPrivateField, UtilityClass, UtilityClassCanBeEnum, VarargParameter
- **Error Prone:** UnnecessaryFinal, YodaCondition
- **PMD 7:** AtLeastOneConstructor, AvoidInstantiatingObjectsInLoops, AvoidUsingVolatile, CallSuperInConstructor, CommentDefaultAccessModifier, CommentSize, DataClass, DoNotUseThreads, InvalidJavaBean, LocalVariableDeclarationShouldBeAtStartOfBlock, NullAssignment, OnlyOneReturn, UseConcurrentHashMap, UseObjectForClearerAPI
- **Apache NetBeans java.hints:** Access of Private Field of Another Object (encapsulation), Collections without initial capacity (performance), Final class (class_structure), Final method (class_structure), Marker interface (class_structure), No loggers (logging), No-op method in abstract class (class_structure), Organize members (class_structure), Package Visible Inner Class (encapsulation), Protected Inner Class (encapsulation), Public Inner Class (encapsulation), StringBuilder without initial capacity (performance), clone() does not throw CloneNotSupportedException (bugs)
- **Picnic Error Prone Support:** AssertJBigDecimalRules, AssertJBigIntegerRules, AssertJByteRules (Rest), AssertJDoubleRules (Rest), AssertJFloatRules (Rest), AssertJInstantRules (Rest), AssertJIntegerRules (Rest), AssertJLongRules (Rest), AssertJShortRules (Rest), AssertJThrowingCallableRules (Rest), ImmutableListRules (Rest), ImmutableMapRules (Rest), ImmutableSetRules (Rest), LexicographicalAnnotationAttributeListing, LexicographicalAnnotationListing, LexicographicalPermitsListing, PrimitiveRules (Rest)
- **Palantir baseline-error-prone:** DangerousThreadPoolExecutorUsage, ShutdownHook
- **Modernizer Maven Plugin:** Guava Iterables → Stream-API
- **Eclipse JDT Compiler:** COMPILER_PB_UNQUALIFIED_FIELD_ACCESS

</details>

<details><summary>ALT (49)</summary>

- **IntelliJ IDEA Community:** AbstractMethodWithMissingImplementations, Annotation, AnnotationClass, ArrayLengthInLoopCondition, CheckForOutOfMemoryOnLargeArrayAllocation, ConfusingMainMethod, ConnectionResource, CustomSecurityManager, EnumClass, ForEachWithRecordPatternCanBeUsed, ForeachStatement, JavacQuirks, Junit4Converter, LengthOneStringsInConcatenation, LoggingStringTemplateAsArgument, MultiplyOrDivideByPowerOfTwo, PrivateMemberAccessBetweenOuterAndInnerClass, RecordStoreResource, RedundantEmbeddedExpression, StringTemplateMigration, StringTemplateReverseMigration, SuperTearDownInFinally, SystemRunFinalizersOnExit, SystemSetSecurityManager, ThreadDeathRethrown, UNCHECKED_WARNING, UseOfAWTPeerClass
- **Error Prone:** CannotMockFinalClass, CannotMockMethod, JUnit3FloatingPointComparisonWithoutDelta, JUnit3TestNotRun, JUnit4ClassUsedInJUnit3, VarTypeName
- **PMD 7:** AccessorClassGeneration, AccessorMethodGeneration, AvoidAssertAsIdentifier, AvoidEnumAsIdentifier, JUnit4SuitesShouldUseSuiteAnnotation, JUnitSpelling, JUnitStaticSuite
- **javac `-Xlint`:** compiler.warn.invalid.yield, compiler.warn.underscore.as.identifier
- **Eclipse JDT Compiler:** COMPILER_PB_ASSERT_IDENTIFIER, COMPILER_PB_ENUM_IDENTIFIER, COMPILER_PB_INCONSISTENT_NULL_CHECK, COMPILER_PB_INVALID_IMPORT, COMPILER_PB_SYNTHETIC_ACCESS_EMULATION, COMPILER_PB_UNREACHABLE_CODE, COMPILER_PB_UNSAFE_TYPE_OPERATION

</details>

<details><summary>FMT (15)</summary>

- **IntelliJ IDEA Community:** JavaLangImport, JavadocBlankLines, MISSORTED_IMPORTS, MissortedModifiers, SamePackageImport, SuspiciousIndentAfterControlStatement, UNUSED_IMPORT
- **Error Prone:** RemoveUnusedImports
- **PMD 7:** UnnecessaryImport
- **Apache NetBeans java.hints:** DN_indentation (bugs), Import From The Same Package (imports), Import From java.lang Package (imports), Organize imports (imports), Unused Import (imports)
- **Eclipse JDT Compiler:** COMPILER_PB_UNUSED_IMPORT

</details>

<details><summary>INTERN (30)</summary>

- **IntelliJ IDEA Community:** StringToUpperWithoutLocale2
- **Error Prone:** ASTHelpersSuggestions, BugPatternNaming, InjectOnBugCheckers, MemoizeConstantVisitorStateLookups, MisformattedTestData, MissingRefasterAnnotation, PreferInstanceofOverGetKind, SymbolToString, TreeToString, TypeEquals, TypeToString, UnnecessaryMethodInvocationMatcher
- **PMD 7:** UnnecessaryWarningSuppression
- **Apache NetBeans java.hints:** AbstractProcessor.getSupportedAnnotationTypes() is overridden (rules15), negative instance of (NegativeInstanceOfTest.hint)
- **Picnic Error Prone Support:** BugCheckerRules, CanonicalClassNameUsage, ClassRules (Rest), SuggestedFixRules
- **Palantir baseline-error-prone:** BugCheckerAutoService
- **javac `-Xlint`:** options, processing
- **Eclipse JDT Compiler:** COMPILER_PB_FATAL_OPTIONAL_ERROR, COMPILER_PB_MAX_PER_UNIT, COMPILER_PB_SUPPRESS_OPTIONAL_ERRORS, COMPILER_PB_SUPPRESS_WARNINGS, COMPILER_PB_SUPPRESS_WARNINGS_NOT_FULLY_ANALYSED, COMPILER_PB_UNHANDLED_WARNING_TOKEN, COMPILER_PB_UNUSED_WARNING_TOKEN

</details>
