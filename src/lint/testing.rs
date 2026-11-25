//! Rules of group T: test code (JUnit, AssertJ, Mockito).

mod assert_argument_order;
mod assert_equals_on_arrays;
mod assert_statement_in_test;
mod assertj_method_result;
mod incomplete_mockito_call;
mod malformed_test;
mod mixed_junit_versions;
mod non_static_class_fixture;
mod self_assertion;
mod shared;
mod simplifiable_assertion;
mod static_mock;
mod swallowed_assertion;
mod test_without_annotation;

use super::Rule;

use jtk_parser::ast::Tag;

/// The checks on the AST, with the kinds of trees each checks.
pub(super) const TREE_CHECKS: &[(Rule, &[Tag], super::tree::TreeCheck)] = &[
    (Rule::TestWithoutAnnotation, &[Tag::Class], test_without_annotation::check),
    (Rule::NonStaticClassFixture, &[Tag::Method], non_static_class_fixture::check),
    (Rule::MalformedTest, &[Tag::Method], malformed_test::check),
    (Rule::MixedJunitVersions, &[Tag::Class], mixed_junit_versions::check),
    (Rule::SwallowedAssertion, &[Tag::Try], swallowed_assertion::check),
    (Rule::AssertArgumentOrder, &[Tag::MethodInvocation], assert_argument_order::check),
    (Rule::AssertEqualsOnArrays, &[Tag::MethodInvocation], assert_equals_on_arrays::check),
    (Rule::SelfAssertion, &[Tag::MethodInvocation], self_assertion::check),
    (Rule::SimplifiableAssertion, &[Tag::MethodInvocation], simplifiable_assertion::check),
    (Rule::AssertStatementInTest, &[Tag::Assert], assert_statement_in_test::check),
    (Rule::AssertjMethodResult, &[Tag::MethodInvocation], assertj_method_result::check),
    (Rule::StaticMock, &[Tag::Vars], static_mock::check),
    (Rule::IncompleteMockitoCall, &[Tag::MethodInvocation], incomplete_mockito_call::check),
];
