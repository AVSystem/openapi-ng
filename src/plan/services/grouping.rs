//! Grouping operations into services.

use indexmap::IndexMap;

use crate::{
  api_model::canonical::OperationDef,
  error::{Diagnostic, Reporter},
  identifier::MethodName,
};

pub(crate) type GroupedOperations<'a> = Vec<(String, Vec<(&'a OperationDef, MethodName)>)>;

/// Groups operations by their resolved group name, resolving each
/// operation's method name in the same pass.
///
/// Groups and their members come back in the order the operations were
/// discovered, unsorted.
pub(crate) fn group_operations<'a>(
  operations: &'a [OperationDef],
  resolver: &crate::plan::naming::NamingResolver,
  reporter: &Reporter,
) -> Result<GroupedOperations<'a>, Diagnostic> {
  // `IndexMap` keeps the groups in discovery order.
  operations
    .iter()
    .try_fold(
      IndexMap::<String, Vec<(&'a OperationDef, MethodName)>>::new(),
      |mut groups, operation| {
        let group_name = resolver.group(operation, reporter)?;
        let method_name = resolver.method_name(operation, reporter)?;
        groups
          .entry(group_name)
          .or_default()
          .push((operation, method_name));
        Ok(groups)
      },
    )
    .map(|groups| groups.into_iter().collect())
}

#[cfg(test)]
mod tests {
  mod grouper {
    use crate::{
      api_model::{
        canonical::{HttpMethod, OperationDef, RequestDef, ResponseContent},
        schema::{SchemaScalar, SchemaType},
      },
      plan::{naming::NamingResolver, services::group_operations},
      test_support::test_reporter,
    };

    fn operation(id: &str, tags: Vec<&str>) -> OperationDef {
      OperationDef {
        operation_id: id.to_string(),
        tags: tags.into_iter().map(str::to_string).collect(),
        method: HttpMethod::Get,
        path: format!("/{id}"),
        request: RequestDef::default(),
        response: Some(ResponseContent::Json(Some(SchemaType::Scalar(
          SchemaScalar::Boolean,
        )))),
        errors: Vec::new(),
        description: None,
        deprecated: false,
      }
    }

    #[test]
    fn tag_first_operation_grouper_preserves_group_and_operation_discovery_order() {
      let operations = [
        operation("listPets", vec!["Pet"]),
        operation("listAdoptions", vec!["Adoption"]),
        operation("getPet", vec!["Pet"]),
      ];
      let ctx = test_reporter();
      let resolver = NamingResolver::default();
      let groups = group_operations(&operations, &resolver, &ctx).expect("grouping succeeds");

      assert_eq!(
        groups
          .iter()
          .map(|(name, _)| name.as_str())
          .collect::<Vec<_>>(),
        vec!["Pet", "Adoption"]
      );
      assert_eq!(
        groups[0]
          .1
          .iter()
          .map(|(operation, _method_name)| operation.operation_id.as_str())
          .collect::<Vec<_>>(),
        vec!["listPets", "getPet"]
      );
    }

    #[test]
    fn tagless_operations_fall_back_to_path_derived_group_with_default_resolver() {
      // The previous `tag_first_operation_grouper_rejects_tagless_operations`
      // test asserted a policy violation; with the configurable naming
      // engine, the default `group` rule falls back to
      // `pascalCase(pathSegments[0])` when tags are missing.
      let ctx = test_reporter();
      let resolver = NamingResolver::default();
      let ops = [operation("listPets", Vec::new())];
      let groups = group_operations(&ops, &resolver, &ctx)
        .expect("default resolver groups by path segment when tags are absent");
      assert_eq!(groups.len(), 1);
      assert_eq!(groups[0].0, "ListPets");
    }
  }
}
