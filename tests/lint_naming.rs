//! Rules of group N (naming) in their own modules. The expected positions are those Checkstyle 14.4.0 reports.

mod common;
use common::{messages, violations};

#[test]
fn abbreviation_in_name() {
    let source = "class XMLHTTPRequest {
    int parseXMLDoc;
    int fieldABCDE;
    static int staticABCDE;
    final int finalABCDE = 1;
    void methodABCDE(int paramABCDE) {}
    @Override public String toSTRINGG() { return \"\"; }
    interface I { int CONST_ABCDE = 1; }
    void f(Object o) {
        if (o instanceof String patternABCDE) {}
        int localABCDE = 0;
        final int finalLocalABCDE = 0;
        java.util.function.BiFunction<Integer, Integer, Integer> add = (lambdaABCDE, b) -> b;
        try { } catch (RuntimeException catchABCDE) { }
    }
    record RecordABCDE(int componentABCDE) {}
    enum E { CONSTANTABCDE }
    int A_ABCD_X;
}
";
    assert_eq!(
        violations("N013", source),
        [
            "1:7 N013",
            "3:9 N013",
            "6:10 N013",
            "6:26 N013",
            "10:33 N013",
            "11:13 N013",
            "13:73 N013",
            "14:41 N013",
            "16:12 N013",
            "16:28 N013"
        ]
    );
    assert_eq!(
        messages("N013", source)[0],
        "Abbreviation in name 'XMLHTTPRequest' must contain no more than '4' consecutive capital letters"
    );
}

#[test]
fn illegal_identifier() {
    let source = "class A$B {
    int var;
    void f$() {}
    enum E { X$ }
    void g() {
        java.util.function.Function<String, String> f = var -> var;
        int ok = 1;
        int $ = 2;
    }
}
";
    assert_eq!(
        violations("N014", source),
        ["1:7 N014", "2:9 N014", "3:10 N014", "4:14 N014", "6:57 N014", "8:13 N014"]
    );
    assert_eq!(messages("N014", source)[0], r"Name 'A$B' must match pattern '^(?!var$|\S*\$)\S+$'");
}
