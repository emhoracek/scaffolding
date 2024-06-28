use convert_case::{Case, Casing};
use handlebars::{
    handlebars_helper, Context, Handlebars, Helper, HelperResult, Output, RenderContext,
};
use package_manager_command::register_package_manager_command;
use serde_json::Value;

pub mod filter;
pub mod merge;
pub mod package_manager_command;
pub mod uniq_lines;

use filter::register_filter;
use merge::register_merge;
use uniq_lines::register_uniq_lines;

pub fn register_helpers(h: Handlebars) -> Handlebars {
    let h = register_concat_helper(h);
    let h = register_contains_helper(h);
    let h = register_includes_helper(h);
    let h = register_case_helpers(h);
    let h = register_replace_helper(h);
    let h = register_pluralize_helpers(h);
    let h = register_merge(h);
    let h = register_uniq_lines(h);
    let h = register_filter(h);
    let h = register_package_manager_command(h);

    h
}

pub fn register_concat_helper(mut h: Handlebars) -> Handlebars {
    h.register_helper(
        "concat",
        Box::new(
            |h: &Helper,
             _r: &Handlebars,
             _: &Context,
             _rc: &mut RenderContext,
             out: &mut dyn Output|
             -> HelperResult {
                let result = h
                    .params()
                    .iter()
                    .map(|p| p.render())
                    .collect::<Vec<String>>()
                    .join("");

                out.write(result.as_ref())?;
                Ok(())
            },
        ),
    );

    h
}

pub fn register_contains_helper(mut h: Handlebars) -> Handlebars {
    handlebars_helper!(contains: |list: Option<Vec<Value>>, value: Value| list.is_some() && list.unwrap().contains(&value));
    h.register_helper("contains", Box::new(contains));

    h
}

pub fn register_includes_helper(mut h: Handlebars) -> Handlebars {
    handlebars_helper!(includes: |string: String, substring: String| string.contains(&substring));
    h.register_helper("includes", Box::new(includes));

    h
}

pub fn register_replace_helper(mut h: Handlebars) -> Handlebars {
    handlebars_helper!(replace: |s: String, pattern: String, replaced_by: String| s.replace(&pattern, replaced_by.as_str()));
    h.register_helper("replace", Box::new(replace));

    h
}

pub fn register_pluralize_helpers(mut h: Handlebars) -> Handlebars {
    handlebars_helper!(singular: |s: String| pluralizer::pluralize(s.as_str(), 1, false));
    h.register_helper("singular", Box::new(singular));
    handlebars_helper!(plural: |s: String| pluralizer::pluralize(s.as_str(), 2, false));
    h.register_helper("plural", Box::new(plural));

    h
}

pub fn register_case_helpers(mut h: Handlebars) -> Handlebars {
    handlebars_helper!(title_case: |s: String| s.to_case(Case::Title));
    h.register_helper("title_case", Box::new(title_case));

    handlebars_helper!(lower_case: |s: String| s.to_case(Case::Lower));
    h.register_helper("lower_case", Box::new(lower_case));

    handlebars_helper!(snake_case: |s: String| s.to_case(Case::Snake));
    h.register_helper("snake_case", Box::new(snake_case));

    handlebars_helper!(kebab_case: |s: String| s.to_case(Case::Kebab));
    h.register_helper("kebab_case", Box::new(kebab_case));

    handlebars_helper!(camel_case: |s: String| s.to_case(Case::Camel));
    h.register_helper("camel_case", Box::new(camel_case));

    handlebars_helper!(pascal_case: |s: String| s.to_case(Case::Pascal));
    h.register_helper("pascal_case", Box::new(pascal_case));

    h
}
