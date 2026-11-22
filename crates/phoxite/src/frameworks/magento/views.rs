//! What a Magento template is given.
//!
//! A `.phtml` file of a `templates` folder (a module's
//! `view/<area>/templates`, a theme's `Vendor_Module/templates`) is rendered
//! by a block, with these variables:
//!
//! | Variable | Is |
//! | --- | --- |
//! | `$block` | the class of the block the layout gives the template (`<block class="…" template="Vendor_Module::x.phtml"/>`, a `referenceBlock` that sets its template); every such class when several blocks render it; `Magento\Framework\View\Element\Template` when the layout names none |
//! | `$this` | the template engine, `Magento\Framework\View\TemplateEngine\Php` |
//! | `$escaper` | `Magento\Framework\Escaper` |
//! | `$secureRenderer` | `Magento\Framework\View\Helper\SecureHtmlRenderer` |
//! | `$viewModel` | the class of the block's `view_model` argument, when the layout gives it one |
//!
//! A block declared for the template's own area is preferred to one of
//! another area. A `/** @var \Vendor\Module\Block\X $block */` at the top of
//! the template still wins: these are what the variable is without one. The
//! types come to the template's own inference through
//! [`Declarations::view_variable`](crate::types::Declarations::view_variable).

use std::path::Path;
use std::sync::Arc;

use crate::engine::PhpEngine;
use crate::types::Type;

pub const TEMPLATE_BLOCK: &str = "Magento\\Framework\\View\\Element\\Template";
const ENGINE: &str = "Magento\\Framework\\View\\TemplateEngine\\Php";
const ESCAPER: &str = "Magento\\Framework\\Escaper";
const SECURE_RENDERER: &str = "Magento\\Framework\\View\\Helper\\SecureHtmlRenderer";

impl PhpEngine {
    /// Whether `path` is a template of the Magento project.
    pub(crate) fn is_magento_template(&self, path: &Path) -> bool {
        self.magento().is_template(path)
    }

    /// The variables the template at `path` is given, with their types.
    pub(crate) fn magento_view_variables(&self, path: &Path) -> Arc<Vec<(String, Type)>> {
        let count = self.edit_count();
        if let Some((at, known)) = self.magento_views_cache().lock().get(path)
            && *at == count
        {
            return known.clone();
        }
        let project = self.magento();
        let mut classes: Vec<String> = Vec::new();
        let mut view_models: Vec<String> = Vec::new();
        if let Some((id, area)) = project.template_id(path) {
            let blocks = project.blocks_of(&id);
            // The blocks of the template's area, when it has some.
            let own: Vec<_> = blocks.iter().filter(|block| block.area == area || block.area == "base").collect();
            let rendering: Vec<_> = if own.is_empty() { blocks.iter().collect() } else { own };
            for block in rendering {
                // A reference names no class: the block it refers to has it.
                let class = block.class.clone().or_else(|| {
                    let declared =
                        project.blocks.iter().find(|other| other.name == block.name && other.class.is_some());
                    declared.and_then(|other| other.class.clone())
                });
                // A virtual type is its class.
                let class = class.map(|class| project.virtual_class(&class).unwrap_or(class));
                if let Some(class) = class
                    && !classes.contains(&class)
                {
                    classes.push(class);
                }
                let given = project.blocks.iter().filter(|other| other.name == block.name);
                for view_model in given.filter_map(|other| other.view_model.clone()) {
                    if !view_models.contains(&view_model) {
                        view_models.push(view_model);
                    }
                }
            }
        }
        if classes.is_empty() {
            classes.push(TEMPLATE_BLOCK.to_owned());
        }
        let union =
            |classes: &[String]| Type::union(classes.iter().map(|class| Type::named(class)).collect::<Vec<Type>>());
        let mut variables = vec![
            ("block".to_owned(), union(&classes)),
            ("this".to_owned(), Type::named(ENGINE)),
            ("escaper".to_owned(), Type::named(ESCAPER)),
            ("secureRenderer".to_owned(), Type::named(SECURE_RENDERER)),
        ];
        if !view_models.is_empty() {
            variables.push(("viewModel".to_owned(), union(&view_models)));
        }
        let variables = Arc::new(variables);
        self.magento_views_cache().lock().insert(path.to_path_buf(), (count, variables.clone()));
        variables
    }
}
