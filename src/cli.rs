use crate::error::{ScaffoldError, ScaffoldResult};
use crate::file_tree::{
    build_file_tree, file_content, load_directory_into_memory, map_file, FileTree,
};
use crate::scaffold::app::cargo::exec_metadata;
use crate::scaffold::app::nix::setup_nix_developer_environment;
use crate::scaffold::app::AppFileTree;
use crate::scaffold::collection::{scaffold_collection, CollectionType};
use crate::scaffold::dna::{scaffold_dna, DnaFileTree};
use crate::scaffold::entry_type::crud::{parse_crud, Crud};
use crate::scaffold::entry_type::definitions::{
    Cardinality, EntryTypeReference, FieldDefinition, FieldType, Referenceable,
};
use crate::scaffold::entry_type::{fields::parse_fields, scaffold_entry_type};
use crate::scaffold::example::{choose_example, Example};
use crate::scaffold::link_type::scaffold_link_type;
use crate::scaffold::web_app::scaffold_web_app;
use crate::scaffold::web_app::uis::UiFramework;
use crate::scaffold::zome::utils::{select_integrity_zomes, select_scaffold_zome_options};
use crate::scaffold::zome::{
    integrity_zome_name, scaffold_coordinator_zome, scaffold_coordinator_zome_in_path,
    scaffold_integrity_zome, scaffold_integrity_zome_with_path, scaffold_zome_pair, ZomeFileTree,
};
use crate::templates::example::scaffold_example;
use crate::templates::ScaffoldedTemplate;
use crate::utils::{
    check_case, check_no_whitespace, input_no_whitespace, input_with_case, input_yes_or_no,
};

use build_fs_tree::{dir, Build, MergeableFileSystemTree};
use colored::Colorize;
use convert_case::Case;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Input;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::{env, fs};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct HcScaffold {
    #[structopt(short, long)]
    /// The template to use for the scaffold command
    /// Can either be an option from the built-in templates: "vanilla", "vue", "lit", "svelte", "headless"
    /// Or a path to a custom template
    template: Option<String>,

    #[structopt(subcommand)]
    command: HcScaffoldCommand,
}

/// The list of subcommands for `hc scaffold`
#[derive(Debug, StructOpt)]
#[structopt(setting = structopt::clap::AppSettings::InferSubcommands)]
pub enum HcScaffoldCommand {
    /// Scaffold a new, empty web app
    WebApp {
        /// Name of the app to scaffold
        name: Option<String>,

        /// Description of the app to scaffold
        description: Option<String>,

        #[structopt(long)]
        /// Whether to setup the holonix development environment for this web-app
        setup_nix: bool,

        #[structopt(long = "holo", hidden = true)]
        holo_enabled: bool,

        /// Whether to skip setting up an initial DNA and it's zome(s) after the web app is scaffolded
        #[structopt(short = "F")]
        disable_fast_track: bool,
    },
    /// Manage custom templates
    Template(HcScaffoldTemplate),
    /// Scaffold a DNA into an existing app
    Dna {
        #[structopt(long)]
        /// Name of the app in which you want to scaffold the DNA
        app: Option<String>,

        /// Name of the DNA being scaffolded
        name: Option<String>,
    },
    /// Scaffold one or multiple zomes into an existing DNA
    Zome {
        #[structopt(long)]
        /// Name of the dna in which you want to scaffold the zome
        dna: Option<String>,

        /// Name of the zome being scaffolded
        name: Option<String>,

        #[structopt(long, parse(from_os_str))]
        /// Scaffold an integrity zome at the given path
        integrity: Option<PathBuf>,

        #[structopt(long, parse(from_os_str))]
        /// Scaffold a coordinator zome at the given path
        coordinator: Option<PathBuf>,
    },
    /// Scaffold an entry type and CRUD functions into an existing zome
    EntryType {
        #[structopt(long)]
        /// Name of the dna in which you want to scaffold the zome
        dna: Option<String>,

        #[structopt(long)]
        /// Name of the integrity zome in which you want to scaffold the entry definition
        zome: Option<String>,

        /// Name of the entry type being scaffolded
        name: Option<String>,

        #[structopt(long)]
        /// Whether this entry type should be refereced with its "EntryHash" or its "ActionHash"
        /// If referred to by "EntryHash", the entries can't be updated or deleted
        reference_entry_hash: Option<bool>,

        #[structopt(long, parse(try_from_str = parse_crud))]
        /// The Create, "Read", "Update", and "Delete" zome call functions that should be scaffolded for this entry type
        /// If "--reference-entry-hash" is "true", only "Create" and "Read" will be scaffolded
        crud: Option<Crud>,

        #[structopt(long)]
        /// Whether to create a link from the original entry to each update action
        /// Only applies if update is selected in the "crud" argument
        link_from_original_to_each_update: Option<bool>,

        #[structopt(long, value_delimiter = ",", parse(try_from_str = parse_fields))]
        /// The fields that the entry type struct should contain
        /// Grammar: <FIELD_NAME>:<FIELD_TYPE>:<WIDGET>:<LINKED_FROM> , (widget and linked_from are optional)
        /// Eg. "title:String:TextField" , "posts_hashes:Vec\<ActionHash\>::Post"
        fields: Option<Vec<FieldDefinition>>,

        #[structopt(long)]
        /// Skips UI generation for this entry-type, overriding any specified widgets in the --fields option.
        ///
        /// **WARNING**: Opting out of UI generation for an entry type but not for other entry-types, link-types or collections associated with it
        /// may result in potential UI inconsistencies. Specifically, UI elements intended for associated entry-types, link-types or collections could inadvertently reference or expect
        /// elements from the skipped entry type.
        ///
        /// If you choose to use this flag, consider applying it consistently across all entry-type, link-type and collection scaffolds
        /// within your project to ensure UI consistency and avoid the outlined integration complications.
        no_ui: bool,
    },
    /// Scaffold a link type and its appropriate zome functions into an existing zome
    LinkType {
        #[structopt(long)]
        /// Name of the dna in which you want to scaffold the zome
        dna: Option<String>,

        #[structopt(long)]
        /// Name of the integrity zome in which you want to scaffold the link type
        zome: Option<String>,

        #[structopt(parse(try_from_str = Referenceable::from_str))]
        /// Entry type (or agent role) used as the base for the links
        from_referenceable: Option<Referenceable>,

        #[structopt(parse(try_from_str = Referenceable::from_str))]
        /// Entry type (or agent role) used as the target for the links
        to_referenceable: Option<Referenceable>,

        #[structopt(long)]
        /// Whether to create the inverse link, from the "--to-referenceable" entry type to the "--from-referenceable" entry type
        bidirectional: Option<bool>,

        #[structopt(long)]
        /// Whether this link type can be deleted
        delete: Option<bool>,

        #[structopt(long)]
        /// Skips UI generation for this link-type.
        no_ui: bool,
    },
    /// Scaffold a collection of entries in an existing zome
    Collection {
        #[structopt(long)]
        /// Name of the dna in which you want to scaffold the zome
        dna: Option<String>,

        #[structopt(long)]
        /// Name of the integrity zome in which you want to scaffold the link type
        zome: Option<String>,

        /// Collection type: "global" or "by-author"
        collection_type: Option<CollectionType>,

        /// Collection name, just to differentiate it from other collections
        collection_name: Option<String>,

        #[structopt(parse(try_from_str = EntryTypeReference::from_str))]
        /// Entry type that is going to be added to the collection
        entry_type: Option<EntryTypeReference>,

        #[structopt(long)]
        /// Skips UI generation for this collection.
        no_ui: bool,
    },
    Example {
        /// Name of the example to scaffold. One of ['hello-world', 'forum'].
        example: Option<Example>,

        #[structopt(long = "holo", hidden = true)]
        holo_enabled: bool,
    },
}

impl HcScaffold {
    pub async fn run(self) -> anyhow::Result<()> {
        let current_dir = std::env::current_dir()?;
        let template_config = get_template_config(&current_dir)?;
        let template = match (&template_config, &self.template) {
            (Some(config), Some(template)) if &config.template != template => {
                return Err(ScaffoldError::InvalidArguments(format!(
                "The value {} passed with `--template` does not match the template the web-app was scaffolded with: {}",
                template.italic(),
                config.template.italic(),
            )).into())
            }
            // Only read from config if the template is inbuilt and not a path
            (Some(config), _)  if !Path::new(&config.template).exists() => Some(&config.template),
            (_, t) => t.as_ref(),
        };

        // Given a template either passed via the --template flag or retreived via the hcScaffold config,
        // get the template file tree and the ui framework name or custom template path
        let (template, template_file_tree) = match template {
            Some(template) => match template.to_lowercase().as_str() {
                "lit" | "svelte" | "vanilla" | "vue" | "headless" => {
                    let ui_framework = UiFramework::from_str(template)?;
                    (ui_framework.name(), ui_framework.template_filetree()?)
                }
                custom_template_path if Path::new(custom_template_path).exists() => {
                    let templates_dir = current_dir.join(custom_template_path);
                    (
                        custom_template_path.to_string(),
                        load_directory_into_memory(&templates_dir)?,
                    )
                }
                path => return Err(ScaffoldError::PathNotFound(PathBuf::from(path)).into()),
            },
            None => {
                let ui_framework = match self.command {
                    HcScaffoldCommand::WebApp { .. } => UiFramework::choose()?,
                    HcScaffoldCommand::Example { ref example, .. } => match example {
                        Some(Example::HelloWorld) => UiFramework::Vanilla,
                        _ => UiFramework::choose_non_vanilla()?,
                    },
                    _ => {
                        let file_tree = load_directory_into_memory(&current_dir)?;
                        UiFramework::try_from(&file_tree)?
                    }
                };
                (ui_framework.name(), ui_framework.template_filetree()?)
            }
        };

        match self.command {
            HcScaffoldCommand::WebApp {
                name,
                description,
                setup_nix,
                holo_enabled,
                disable_fast_track,
            } => {
                let name = match name {
                    Some(n) => {
                        check_no_whitespace(&n, "app name")?;
                        n
                    }
                    None => input_no_whitespace("App name (no whitespaces):")?,
                };

                let app_folder = current_dir.join(&name);

                if app_folder.as_path().exists() {
                    return Err(ScaffoldError::FolderAlreadyExists(app_folder.clone()))?;
                }

                if file_content(&template_file_tree, &PathBuf::from("web-app/README.md.hbs"))
                    .is_err()
                {
                    return Err(ScaffoldError::MalformedTemplate(
                        "Template does not contain a README.md.hbs file in its \"web-app\" directory"
                            .to_string(),
                    ))?;
                }

                let setup_nix = if setup_nix {
                    setup_nix
                } else {
                    input_yes_or_no(
                        "Do you want to set up the holonix development environment for this project?", 
                        Some(true)
                    )?
                };

                let ScaffoldedTemplate {
                    file_tree,
                    next_instructions,
                } = scaffold_web_app(
                    name.clone(),
                    description,
                    !setup_nix,
                    &template_file_tree,
                    holo_enabled,
                )?;

                let file_tree = write_scaffold_config(file_tree, &template)?;

                build_file_tree(dir! {&name => file_tree}, ".")?;

                let mut nix_instructions = "";

                let app_dir = std::env::current_dir()?.join(&name);
                if setup_nix {
                    if let Err(err) = setup_nix_developer_environment(&app_dir) {
                        fs::remove_dir_all(&app_dir)?;
                        return Err(err)?;
                    }
                    nix_instructions = "\n  nix develop";
                }

                if !disable_fast_track
                    && input_yes_or_no("Do you want to scaffold an initial DNA? (y/n)", None)?
                {
                    env::set_current_dir(PathBuf::from(&name))?;
                    // prompt to scaffold DNA
                    let dna_name = input_with_case("Initial DNA name (snake_case):", Case::Snake)?;
                    let file_tree = load_directory_into_memory(&current_dir.join(&name))?;
                    let app_file_tree = AppFileTree::get_or_choose(file_tree, &Some(name.clone()))?;
                    let ScaffoldedTemplate { file_tree, .. } =
                        scaffold_dna(app_file_tree, &template_file_tree, &dna_name)?;

                    if input_yes_or_no("Do you want to scaffold an initial coordinator/integrity zome pair for your DNA? (y/n)", None)? {
                            scaffold_zome_pair(file_tree, template_file_tree, &dna_name)?;
                            println!("Coordinator/integrity zome pair scaffolded.")
                        } else {
                            build_file_tree(file_tree, ".")?;
                            println!("DNA scaffolded.");
                        }
                }

                setup_git_environment(&app_dir)?;

                println!("\nYour Web hApp {} has been scaffolded!", name.italic());

                if let Some(i) = next_instructions {
                    println!("\n{}", i);
                } else {
                    let dna_instructions = if disable_fast_track {
                        r#"

- Get your project to compile by adding a DNA and then following the next insturctions to add a zome to that DNA:

  hc scaffold dna"#
                    } else {
                        ""
                    };
                    println!(
                        r#"
This skeleton provides the basic structure for your Holochain web application.
The UI is currently empty; you will need to import necessary components into the top-level app component to populate it. 

Here's how you can get started with developing your application:

- Set up your development environment:

  cd {name}{nix_instructions}
  npm install {dna_instructions}

- Enhance your app by executing further hc scaffold commands to add more features.

- Then, at any point in time you can start your application with:

  npm start"#,
                    );
                }
            }
            HcScaffoldCommand::Template(template) => template.run(template_file_tree)?,
            HcScaffoldCommand::Dna { app, name } => {
                let name = match name {
                    Some(n) => {
                        check_case(&n, "dna name", Case::Snake)?;
                        n
                    }
                    None => input_with_case("DNA name (snake_case):", Case::Snake)?,
                };

                let file_tree = load_directory_into_memory(&current_dir)?;

                let app_file_tree = AppFileTree::get_or_choose(file_tree, &app)?;

                let ScaffoldedTemplate {
                    file_tree,
                    next_instructions,
                } = scaffold_dna(app_file_tree, &template_file_tree, &name)?;

                build_file_tree(file_tree, ".")?;

                println!("\nDNA {} scaffolded!", name.italic());

                if let Some(i) = next_instructions {
                    println!("\n{}", i);
                } else {
                    println!(
                        r#"
Add new zomes to your DNA with:

  hc scaffold zome
"#,
                    );
                }
            }
            HcScaffoldCommand::Zome {
                dna,
                name,
                integrity,
                coordinator,
            } => {
                let file_tree = load_directory_into_memory(&current_dir)?;

                if let Some(n) = name.clone() {
                    check_case(&n, "zome name", Case::Snake)?;
                }

                let (scaffold_integrity, scaffold_coordinator) = match (&integrity, &coordinator) {
                    (None, None) => select_scaffold_zome_options()?,
                    _ => (integrity.is_some(), coordinator.is_some()),
                };

                let name_prompt = match (scaffold_integrity, scaffold_coordinator) {
                    (true, true) => "Enter coordinator zome name (snake_case):\n (The integrity zome will automatically be named '{name of coordinator zome}_integrity')\n",
                    _ => "Enter zome name (snake_case):",
                };

                let name = match name {
                    Some(n) => n,
                    None => input_with_case(name_prompt, Case::Snake)?,
                };

                let mut dna_file_tree = DnaFileTree::get_or_choose(file_tree, &dna)?;
                let dna_manifest_path = dna_file_tree.dna_manifest_path.clone();

                let mut zome_next_instructions: (Option<String>, Option<String>) = (None, None);

                if scaffold_integrity {
                    let integrity_zome_name = match scaffold_coordinator {
                        true => integrity_zome_name(&name),
                        false => name.clone(),
                    };
                    let ScaffoldedTemplate {
                        file_tree,
                        next_instructions,
                    } = scaffold_integrity_zome(
                        dna_file_tree,
                        &template_file_tree,
                        &integrity_zome_name,
                        &integrity,
                    )?;

                    zome_next_instructions.0 = next_instructions;

                    println!(
                        "\nIntegrity zome {} scaffolded!",
                        integrity_zome_name.italic(),
                    );

                    dna_file_tree =
                        DnaFileTree::from_dna_manifest_path(file_tree, &dna_manifest_path)?;
                }

                if scaffold_coordinator {
                    let dependencies = match scaffold_integrity {
                        true => Some(vec![integrity_zome_name(&name)]),
                        false => {
                            let integrity_zomes = select_integrity_zomes(&dna_file_tree.dna_manifest, Some(
                              "Select integrity zome(s) this coordinator zome depends on (SPACE to select/unselect, ENTER to continue):"
                            ))?;
                            Some(integrity_zomes)
                        }
                    };
                    let ScaffoldedTemplate {
                        file_tree,
                        next_instructions,
                    } = scaffold_coordinator_zome(
                        dna_file_tree,
                        &template_file_tree,
                        &name,
                        &dependencies,
                        &coordinator,
                    )?;
                    zome_next_instructions.1 = next_instructions;

                    println!("\nCoordinator zome {} scaffolded!", name.italic());

                    dna_file_tree =
                        DnaFileTree::from_dna_manifest_path(file_tree, &dna_manifest_path)?;
                }

                // TODO: implement scaffold_zome_template
                let file_tree =
                    MergeableFileSystemTree::<OsString, String>::from(dna_file_tree.file_tree());

                // FIXME: avoid cloning
                let f = file_tree.clone();
                file_tree.build(&PathBuf::from("."))?;

                // Execute cargo metadata to set up the cargo workspace in case this zome is the first crate
                exec_metadata(&f)?;

                match zome_next_instructions {
                    (Some(integrity), Some(coordinator)) => {
                        println!("\n{integrity}");
                        println!("\n{coordinator}");
                    }
                    (None, Some(coordinator)) => println!("\n{coordinator}"),
                    (Some(integrity), None) => println!("\n{integrity}"),
                    _ => println!(
                        r#"
Add new entry definitions to your zome with:

  hc scaffold entry-type
"#,
                    ),
                }
            }
            HcScaffoldCommand::EntryType {
                dna,
                zome,
                name,
                crud,
                reference_entry_hash,
                link_from_original_to_each_update,
                fields,
                no_ui,
            } => {
                let file_tree = load_directory_into_memory(&current_dir)?;
                let name = match name {
                    Some(n) => {
                        check_case(&n, "entry type name", Case::Snake)?;
                        n
                    }
                    None => input_with_case("Entry type name (snake_case):", Case::Snake)?,
                };

                let dna_file_tree = DnaFileTree::get_or_choose(file_tree, &dna)?;
                let zome_file_tree = ZomeFileTree::get_or_choose_integrity(dna_file_tree, &zome)?;

                if no_ui {
                    let warning_text = r#"
WARNING: Opting out of UI generation for an this entry-type but not for other entry-types, link-types or collections associated with it
may result in potential UI inconsistencies. Specifically, UI elements intended for associated entry-types, link-types or collections could 
inadvertently reference or expect elements from the skipped entry type."#
                    .yellow();
                    println!("{warning_text}");
                }

                let ScaffoldedTemplate {
                    file_tree,
                    next_instructions,
                } = scaffold_entry_type(
                    zome_file_tree,
                    &template_file_tree,
                    &name,
                    &crud,
                    reference_entry_hash,
                    link_from_original_to_each_update,
                    fields.as_ref(),
                    no_ui,
                )?;

                build_file_tree(file_tree, ".")?;

                println!("\nEntry type {} scaffolded!", name.italic(),);

                if let Some(i) = next_instructions {
                    println!("\n{}", i);
                } else {
                    println!(
                        r#"
Add new collections for that entry type with:

  hc scaffold collection
"#,
                    );
                }
            }
            HcScaffoldCommand::LinkType {
                dna,
                zome,
                from_referenceable,
                to_referenceable,
                delete,
                bidirectional,
                no_ui,
            } => {
                let file_tree = load_directory_into_memory(&current_dir)?;

                let dna_file_tree = DnaFileTree::get_or_choose(file_tree, &dna)?;
                let zome_file_tree = ZomeFileTree::get_or_choose_integrity(dna_file_tree, &zome)?;

                let ScaffoldedTemplate {
                    file_tree,
                    next_instructions,
                } = scaffold_link_type(
                    zome_file_tree,
                    &template_file_tree,
                    &from_referenceable,
                    &to_referenceable,
                    &delete,
                    &bidirectional,
                    no_ui,
                )?;

                build_file_tree(file_tree, ".")?;

                println!("\nLink type scaffolded!");
                if let Some(i) = next_instructions {
                    println!("\n{}", i);
                }
            }
            HcScaffoldCommand::Collection {
                dna,
                zome,
                collection_name,
                collection_type,
                entry_type,
                no_ui,
            } => {
                let file_tree = load_directory_into_memory(&current_dir)?;

                let dna_file_tree = DnaFileTree::get_or_choose(file_tree, &dna)?;
                let zome_file_tree = ZomeFileTree::get_or_choose_integrity(dna_file_tree, &zome)?;

                let name = match collection_name {
                    Some(n) => {
                        check_case(&n, "collection name", Case::Snake)?;
                        n
                    }
                    None => input_with_case(
                        "Collection name (snake_case, eg. \"all_posts\"):",
                        Case::Snake,
                    )?,
                };

                let ScaffoldedTemplate {
                    file_tree,
                    next_instructions,
                } = scaffold_collection(
                    zome_file_tree,
                    &template_file_tree,
                    &name,
                    &collection_type,
                    &entry_type,
                    no_ui,
                )?;

                build_file_tree(file_tree, ".")?;

                println!("\nCollection {} scaffolded!", name.italic());

                if let Some(i) = next_instructions {
                    println!("\n{i}");
                }
            }
            HcScaffoldCommand::Example {
                example,
                holo_enabled,
            } => {
                let example = match example {
                    Some(e) => e,
                    None => choose_example()?,
                };
                let example_name = example.to_string();

                let app_dir = std::env::current_dir()?.join(&example_name);
                if app_dir.as_path().exists() {
                    return Err(ScaffoldError::FolderAlreadyExists(app_dir.clone()))?;
                }

                // Ensure the correct tempalte is used for each example
                if matches!(example, Example::HelloWorld) && template != "vanilla"
                    || matches!(example, Example::Forum) && template == "vanilla"
                {
                    return Err(ScaffoldError::InvalidArguments(format!(
                        "{} example cannot be used with the {} template.",
                        example.to_string().italic(),
                        template.italic(),
                    ))
                    .into());
                }

                // Match on example types
                let file_tree = match example {
                    Example::HelloWorld => {
                        // scaffold web-app
                        let ScaffoldedTemplate { file_tree, .. } = scaffold_web_app(
                            example_name.clone(),
                            Some("A simple 'hello world' application.".to_string()),
                            false,
                            &template_file_tree,
                            holo_enabled,
                        )?;

                        file_tree
                    }
                    Example::Forum => {
                        // scaffold web-app
                        let ScaffoldedTemplate { file_tree, .. } = scaffold_web_app(
                            example_name.clone(),
                            Some("A simple 'forum' application.".to_string()),
                            false,
                            &template_file_tree,
                            holo_enabled,
                        )?;

                        // scaffold dna hello_world
                        let dna_name = "forum";

                        let app_file_tree =
                            AppFileTree::get_or_choose(file_tree, &Some(example_name.clone()))?;
                        let ScaffoldedTemplate { file_tree, .. } =
                            scaffold_dna(app_file_tree, &template_file_tree, dna_name)?;

                        // scaffold integrity zome posts
                        let dna_file_tree =
                            DnaFileTree::get_or_choose(file_tree, &Some(dna_name.to_owned()))?;
                        let dna_manifest_path = dna_file_tree.dna_manifest_path.clone();

                        let integrity_zome_name = "posts_integrity";
                        let integrity_zome_path = PathBuf::new()
                            .join("dnas")
                            .join(dna_name)
                            .join("zomes")
                            .join("integrity");
                        let ScaffoldedTemplate { file_tree, .. } =
                            scaffold_integrity_zome_with_path(
                                dna_file_tree,
                                &template_file_tree,
                                integrity_zome_name,
                                &integrity_zome_path,
                            )?;

                        let dna_file_tree =
                            DnaFileTree::from_dna_manifest_path(file_tree, &dna_manifest_path)?;

                        let coordinator_zome_name = "posts";
                        let coordinator_zome_path = PathBuf::new()
                            .join("dnas")
                            .join(dna_name)
                            .join("zomes")
                            .join("coordinator");
                        let ScaffoldedTemplate { file_tree, .. } =
                            scaffold_coordinator_zome_in_path(
                                dna_file_tree,
                                &template_file_tree,
                                coordinator_zome_name,
                                &Some(vec![integrity_zome_name.to_owned()]),
                                &coordinator_zome_path,
                            )?;

                        // Scaffold the app here to enable ZomeFileTree::from_manifest(), which calls `cargo metadata`
                        MergeableFileSystemTree::<OsString, String>::from(file_tree.clone())
                            .build(&app_dir)?;

                        std::env::set_current_dir(&app_dir)?;

                        let dna_file_tree =
                            DnaFileTree::from_dna_manifest_path(file_tree, &dna_manifest_path)?;

                        let zome_file_tree = ZomeFileTree::get_or_choose_integrity(
                            dna_file_tree,
                            &Some(integrity_zome_name.to_owned()),
                        )?;

                        let post_entry_type_name = "post";

                        let ScaffoldedTemplate { file_tree, .. } = scaffold_entry_type(
                            zome_file_tree,
                            &template_file_tree,
                            "post",
                            &Some(Crud {
                                update: true,
                                delete: true,
                            }),
                            Some(false),
                            Some(true),
                            Some(&vec![
                                FieldDefinition {
                                    field_name: "title".to_string(),
                                    field_type: FieldType::String,
                                    widget: Some("TextField".to_string()),
                                    cardinality: Cardinality::Single,
                                    linked_from: None,
                                },
                                FieldDefinition {
                                    field_name: "content".to_string(),
                                    field_type: FieldType::String,
                                    widget: Some("TextArea".to_string()),
                                    cardinality: Cardinality::Single,
                                    linked_from: None,
                                },
                            ]),
                            false,
                        )?;

                        let dna_file_tree =
                            DnaFileTree::from_dna_manifest_path(file_tree, &dna_manifest_path)?;

                        let zome_file_tree = ZomeFileTree::get_or_choose_integrity(
                            dna_file_tree,
                            &Some("posts_integrity".to_string()),
                        )?;

                        let ScaffoldedTemplate { file_tree, .. } = scaffold_entry_type(
                            zome_file_tree,
                            &template_file_tree,
                            "comment",
                            &Some(Crud {
                                update: false,
                                delete: true,
                            }),
                            Some(false),
                            Some(true),
                            Some(&vec![
                                FieldDefinition {
                                    field_name: "comment".to_string(),
                                    field_type: FieldType::String,
                                    widget: Some("TextArea".to_string()),
                                    cardinality: Cardinality::Single,
                                    linked_from: None,
                                },
                                FieldDefinition {
                                    field_name: "post_hash".to_string(),
                                    field_type: FieldType::ActionHash,
                                    widget: None,
                                    cardinality: Cardinality::Single,
                                    linked_from: Some(Referenceable::EntryType(
                                        EntryTypeReference {
                                            entry_type: post_entry_type_name.to_string(),
                                            reference_entry_hash: false,
                                        },
                                    )),
                                },
                            ]),
                            false,
                        )?;

                        let dna_file_tree =
                            DnaFileTree::from_dna_manifest_path(file_tree, &dna_manifest_path)?;

                        let zome_file_tree = ZomeFileTree::get_or_choose_integrity(
                            dna_file_tree,
                            &Some(integrity_zome_name.to_string()),
                        )?;

                        let ScaffoldedTemplate { file_tree, .. } = scaffold_collection(
                            zome_file_tree,
                            &template_file_tree,
                            "all_posts",
                            &Some(CollectionType::Global),
                            &Some(EntryTypeReference {
                                entry_type: "post".to_string(),
                                reference_entry_hash: false,
                            }),
                            false,
                        )?;

                        file_tree
                    }
                };

                let ScaffoldedTemplate {
                    file_tree,
                    next_instructions,
                } = scaffold_example(file_tree, &template_file_tree, &example)?;

                build_file_tree(file_tree, &app_dir)?;

                // set up nix
                if let Err(err) = setup_nix_developer_environment(&app_dir) {
                    fs::remove_dir_all(&app_dir)?;
                    return Err(err)?;
                }

                setup_git_environment(&app_dir)?;

                println!("\nExample {} scaffolded!", example.to_string().italic());

                if let Some(i) = next_instructions {
                    println!("\n{}", i);
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, StructOpt)]
#[structopt(setting = structopt::clap::AppSettings::InferSubcommands)]
pub enum HcScaffoldTemplate {
    /// Clone the template in use into a new custom template
    Clone {
        #[structopt(long)]
        /// The folder to initialize the template into, will end up at "<TO TEMPLATE>"
        to_template: Option<String>,
    },
}

impl HcScaffoldTemplate {
    pub fn run(self, template_file_tree: FileTree) -> anyhow::Result<()> {
        let target_template = match self.target_template() {
            Some(t) => t,
            None => {
                // Enter template name
                Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Enter new template name:")
                    .interact()?
            }
        };

        let template_file_tree = dir! {
            target_template.clone() => template_file_tree
        };

        let file_tree = MergeableFileSystemTree::<OsString, String>::from(template_file_tree);

        file_tree.build(&PathBuf::from("."))?;

        match self {
            HcScaffoldTemplate::Clone { .. } => {
                println!(
                    r#"Template initialized to folder {:?}
"#,
                    target_template
                );
            }
        }
        Ok(())
    }

    pub fn target_template(&self) -> Option<String> {
        match self {
            HcScaffoldTemplate::Clone {
                to_template: target_template,
                ..
            } => target_template.clone(),
        }
    }
}

fn setup_git_environment(path: &Path) -> ScaffoldResult<()> {
    let output = Command::new("git")
        .stdout(Stdio::inherit())
        .current_dir(path)
        .args(["init", "--initial-branch=main"])
        .output()?;

    if !output.status.success() {
        let output = Command::new("git")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .current_dir(path)
            .args(["init"])
            .output()?;
        if !output.status.success() {
            println!("Warning: error running \"git init\"");
            return Ok(());
        }

        let _output = Command::new("git")
            .current_dir(path)
            .args(["branch", "main"])
            .output()?;
    }

    let output = Command::new("git")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .current_dir(path)
        .args(["add", "."])
        .output()?;

    if !output.status.success() {
        println!("Warning: error running \"git add .\"");
    }
    Ok(())
}

/// Write hcScaffold config to the hApp's root `package.json` file
fn write_scaffold_config(
    mut web_app_file_tree: FileTree,
    template: &str,
) -> ScaffoldResult<FileTree> {
    if Path::new(template).exists() {
        return Ok(web_app_file_tree);
    }
    let config = TemplateConfig {
        template: template.to_owned(),
    };
    let package_json_path = PathBuf::from("package.json");
    map_file(&mut web_app_file_tree, &package_json_path, |c| {
        let original_content = c.clone();
        let json = serde_json::from_str::<Value>(&c)?;
        let json = match json {
            Value::Object(mut o) => {
                o.insert(
                    "hcScaffold".to_owned(),
                    serde_json::to_value(&config).unwrap(),
                );
                o
            }
            _ => return Ok(original_content),
        };
        let json = serde_json::to_value(json)?;
        let json = serde_json::to_string_pretty(&json)?;
        Ok(json)
    })?;
    Ok(web_app_file_tree)
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TemplateConfig {
    template: String,
}

/// Gets template config written to the root `package.json` file when the hApp was
/// originally scaffolded
fn get_template_config(current_dir: &Path) -> ScaffoldResult<Option<TemplateConfig>> {
    let package_json_path = current_dir.join("package.json");
    let Ok(file) = fs::read_to_string(package_json_path) else {
        return Ok(None);
    };
    let file = serde_json::from_str::<Value>(&file)?;
    if let Some(config) = file.get("hcScaffold") {
        let config = serde_json::from_value(config.to_owned())?;
        Ok(Some(config))
    } else {
        Ok(None)
    }
}
