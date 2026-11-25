mod common;

use common::{file_violations, fixed, messages, unsafe_fixed, violations};

// B148

#[test]
fn default_time_zone_is_reported_and_fixed() {
    let source = "import java.time.LocalDate;\nimport java.time.ZoneId;\nclass A {\n  Object a = LocalDate.now();\n  \
                  Object b = java.time.OffsetDateTime.now();\n}\n";
    assert_eq!(violations("B148", source), ["4:14 B148", "5:14 B148"]);
    assert_eq!(messages("B148", source)[0], "LocalDate.now() uses the default time zone");
    assert_eq!(
        fixed("B148", source),
        "import java.time.LocalDate;\nimport java.time.ZoneId;\nclass A {\n  \
         Object a = LocalDate.now(ZoneId.systemDefault());\n  \
         Object b = java.time.OffsetDateTime.now(ZoneId.systemDefault());\n}\n"
    );
}

#[test]
fn default_time_zone_adds_the_import_once_in_sorted_position() {
    let source = "import java.time.OffsetDateTime;\nimport java.util.List;\nclass A {\n  \
                  Object a = OffsetDateTime.now();\n  Object b = OffsetDateTime.now().minusMinutes(15);\n}\n";
    assert_eq!(
        fixed("B148", source),
        "import java.time.OffsetDateTime;\nimport java.time.ZoneId;\nimport java.util.List;\nclass A {\n  \
         Object a = OffsetDateTime.now(ZoneId.systemDefault());\n  \
         Object b = OffsetDateTime.now(ZoneId.systemDefault()).minusMinutes(15);\n}\n"
    );
    // After all imports.
    let source = "import java.time.LocalTime;\nclass A {\n  Object a = LocalTime.now();\n}\n";
    assert_eq!(
        fixed("B148", source),
        "import java.time.LocalTime;\nimport java.time.ZoneId;\nclass A {\n  \
         Object a = LocalTime.now(ZoneId.systemDefault());\n}\n"
    );
    // On demand: the simple name is used already, or gets imported.
    let source = "import java.time.*;\nclass A {\n  Object a = Year.now();\n  ZoneId z;\n}\n";
    assert_eq!(
        fixed("B148", source),
        "import java.time.*;\nclass A {\n  Object a = Year.now(ZoneId.systemDefault());\n  ZoneId z;\n}\n"
    );
    let source = "import java.time.*;\nclass A {\n  Object a = Year.now();\n}\n";
    assert_eq!(
        fixed("B148", source),
        "import java.time.*;\nimport java.time.ZoneId;\nclass A {\n  Object a = Year.now(ZoneId.systemDefault());\n}\n"
    );
}

#[test]
fn default_time_zone_qualifies_zone_id_where_it_cannot_import_it() {
    // Another ZoneId imported, declared, or used without import; no imports at all.
    for (header, body) in [
        ("import java.time.LocalDate;\nimport a.ZoneId;\n", ""),
        ("import java.time.LocalDate;\n", "  class ZoneId {}\n"),
        ("import java.time.LocalDate;\n", "  ZoneId z;\n"),
        ("", ""),
    ] {
        let class = if header.is_empty() { "java.time.LocalDate" } else { "LocalDate" };
        let source = format!("{header}class A {{\n  Object a = {class}.now();\n{body}}}\n");
        let expected =
            format!("{header}class A {{\n  Object a = {class}.now(java.time.ZoneId.systemDefault());\n{body}}}\n");
        assert_eq!(fixed("B148", &source), expected);
    }
}

#[test]
fn default_time_zone_near_misses() {
    for source in [
        // Arguments, Instant, Clock.
        "import java.time.*;\nclass A { Object a = LocalDate.now(clock); Object b = Instant.now(); }",
        "import java.time.*;\nclass A { Object a = LocalDate.now(ZoneId.of(\"UTC\")); Object c = Clock.systemUTC(); }",
        // Not imported, imported from elsewhere, or declared in the file.
        "class A { Object a = LocalDate.now(); }",
        "import org.joda.time.LocalDate;\nclass A { Object a = LocalDate.now(); }",
        "import java.time.*;\nclass A { class LocalDate { static Object now() { return null; } } Object a = LocalDate.now(); }",
        "import java.time.LocalDate;\nclass A { Object a = LocalDate.parse(\"x\"); Object b = now(); }",
    ] {
        assert!(violations("B148", source).is_empty(), "{source}");
    }
    // A comment inside the parentheses: reported without fix.
    let source = "import java.time.LocalDate;\nclass A { Object a = LocalDate.now(/* today */); }";
    assert_eq!(violations("B148", source).len(), 1);
    assert_eq!(fixed("B148", source), source);
}

#[test]
fn default_time_zone_skips_a_class_of_the_same_package_imported_on_demand() {
    let dir = std::env::temp_dir().join(format!("jtk_b148_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Year.java"), "class Year {}").unwrap();
    let path = dir.join("A.java");
    let path = path.to_str().unwrap();
    let source = "import java.time.*;\nclass A { Object a = Year.now(); Object b = LocalDate.now(); }";
    assert_eq!(file_violations("B148", path, source), ["2:45 B148"]);
    // A single-type import wins over the package.
    let source = "import java.time.Year;\nclass A { Object a = Year.now(); }";
    assert_eq!(file_violations("B148", path, source).len(), 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

// S155

const IMPORT: &str = "import java.util.List;\nimport java.util.stream.Collectors;\n";

/// The source with the imports of `List` and `Collectors`, and the class body `body`.
fn class(body: &str) -> String {
    format!("{IMPORT}class A {{\n{body}}}\n")
}

#[test]
fn collector_to_list_is_reported_and_fixed() {
    let source = class(
        "  List<B> f(List<C> s) {\n    return s.stream().map(B::new).collect(Collectors.toList());\n  }\n  \
         Object g(Stream<C> s) {\n    var v = s.collect(java.util.stream.Collectors.toUnmodifiableList());\n    \
         return v;\n  }\n",
    );
    assert_eq!(violations("S155", &source), ["5:12 S155", "8:13 S155"]);
    assert_eq!(
        messages("S155", &source),
        [
            "collect(Collectors.toList()) instead of toList()",
            "collect(Collectors.toUnmodifiableList()) instead of toList()"
        ]
    );
    // Unsafe only.
    assert_eq!(fixed("S155", &source), source);
    assert_eq!(
        unsafe_fixed("S155", &source),
        class(
            "  List<B> f(List<C> s) {\n    return s.stream().map(B::new).toList();\n  }\n  \
             Object g(Stream<C> s) {\n    var v = s.toList();\n    return v;\n  }\n"
        )
    );
}

#[test]
fn collector_to_list_fixes_where_the_element_type_is_known() {
    // Without target type.
    for (code, expected) in [
        ("s.collect(Collectors.toList()).forEach(f);", "s.toList().forEach(f);"),
        ("s.collect(Collectors.toList());", "s.toList();"),
        ("for (Object o : s.collect(Collectors.toList())) {}", "for (Object o : s.toList()) {}"),
    ] {
        let source = class(&format!("  void f() {{\n    {code}\n  }}\n"));
        assert_eq!(unsafe_fixed("S155", &source), class(&format!("  void f() {{\n    {expected}\n  }}\n")));
    }
    // A collection of the same elements, filtered; a field assigned through `this`.
    let source = class(
        "  List<B> bs;\n  A(List<B> bs) {\n    this.bs = bs.stream().filter(b -> b.ok()).sorted().collect(Collectors.toList());\n    \
         bs = bs.stream().distinct().collect(Collectors.toList());\n    \
         List<B> local = bs.stream().collect(Collectors.toList());\n  }\n",
    );
    assert_eq!(
        unsafe_fixed("S155", &source),
        class(
            "  List<B> bs;\n  A(List<B> bs) {\n    this.bs = bs.stream().filter(b -> b.ok()).sorted().toList();\n    \
             bs = bs.stream().distinct().toList();\n    List<B> local = bs.stream().toList();\n  }\n"
        )
    );
}

#[test]
fn collector_to_list_is_not_fixed_where_the_list_may_have_other_elements() {
    for body in [
        // A supertype of the elements, or elements not known.
        "  List<Object> f(List<String> s) {\n    return s.stream().collect(Collectors.toList());\n  }\n",
        "  List<Object> f(List<String> s) {\n    return s.stream().map(String::new).collect(Collectors.toList());\n  }\n",
        "  List<B> f(List<C> s) {\n    return s.stream().map(this::b).collect(Collectors.toList());\n  }\n",
        "  List<B> f(Stream<B> s) {\n    return s.collect(Collectors.toList());\n  }\n",
        "  List<B> f(List<? extends B> s) {\n    return s.stream().collect(Collectors.toList());\n  }\n",
        // In a lambda, an argument, a field of another object.
        "  Supplier<List<B>> f(List<B> s) {\n    return () -> {\n      return s.stream().collect(Collectors.toList());\n    };\n  }\n",
        "  void f(List<B> s) {\n    g(s.stream().collect(Collectors.toList()));\n  }\n",
        "  void f(List<B> s, A a) {\n    a.bs = s.stream().collect(Collectors.toList());\n  }\n",
        // Type arguments, comments.
        "  List<B> f(List<B> s) {\n    return s.stream().collect(Collectors.<B>toList());\n  }\n",
        "  List<B> f(List<B> s) {\n    return s.stream().collect(/* all */ Collectors.toList());\n  }\n",
    ] {
        let source = class(body);
        assert_eq!(violations("S155", &source).len(), 1, "{source}");
        assert_eq!(unsafe_fixed("S155", &source), source);
    }
    // A comment before the call is kept.
    let source = class("  void f() {\n    s /* c */ .collect(Collectors.toList()).size();\n  }\n");
    assert_eq!(unsafe_fixed("S155", &source), class("  void f() {\n    s /* c */ .toList().size();\n  }\n"));
}

#[test]
fn collector_to_list_with_static_imports() {
    let source = "import static java.util.stream.Collectors.toList;\nclass A { void f() { s.collect(toList()); } }";
    assert_eq!(
        unsafe_fixed("S155", source),
        "import static java.util.stream.Collectors.toList;\nclass A { void f() { s.toList(); } }"
    );
    let source = "import static java.util.stream.Collectors.*;\nclass A { Object a = s.collect(toList()); }";
    assert_eq!(violations("S155", source).len(), 1);
    // A method of the file shadows the static import.
    let source = "import static java.util.stream.Collectors.*;\nclass A { Object a = s.collect(toList()); \
                  static Object toList() { return null; } }";
    assert!(violations("S155", source).is_empty());
}

#[test]
fn collector_to_list_near_misses() {
    for source in [
        "class A { Object a = s.collect(Collectors.toList()); }",
        "import a.Collectors;\nclass A { Object a = s.collect(Collectors.toList()); }",
        "import java.util.stream.Collectors;\nclass A { Object a = s.collect(Collectors.toSet()); }",
        "import java.util.stream.Collectors;\nclass A { Object a = s.collect(Collectors.toCollection(ArrayList::new)); }",
        "import java.util.stream.Collectors;\nclass A { Object a = s.collect(Collectors.toList(), x); }",
        "import java.util.stream.Collectors;\nclass A { Object a = collect(Collectors.toList()); }",
        "class A { Object a = s.collect(toList()); }",
    ] {
        assert!(violations("S155", source).is_empty(), "{source}");
    }
}
