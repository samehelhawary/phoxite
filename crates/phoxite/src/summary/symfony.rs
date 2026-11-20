//! What a file tells the Symfony type provider about the rest of the
//! project ([`SymfonyFacts`]): the routes its `#[Route]` attributes name,
//! which entity each Doctrine repository is for, the filters, functions and
//! tests it adds to Twig, and the associations of its Doctrine entities.
//!
//! Recorded for any file (a summary can't know whether Symfony is
//! installed); the index lists the files that have some, and the provider
//! only reads them when it is active. The YAML configuration is read
//! separately ([`crate::symfony::SymfonyConfig`]).

use serde::{Deserialize, Serialize};

use super::Loc;

/// See the module documentation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymfonyFacts {
    /// The named routes of `#[Route]` attributes on methods (with the name
    /// prefix of the class's `#[Route]`), in source order.
    pub routes: Vec<RouteAttribute>,
    pub repositories: Vec<RepositoryEntity>,
    /// What the file adds to Twig, in source order.
    #[serde(default)]
    pub twig: Vec<TwigName>,
    /// The associations of the entities it declares.
    #[serde(default)]
    pub associations: Vec<Association>,
}

impl SymfonyFacts {
    pub fn is_empty(&self) -> bool {
        self.routes.is_empty() && self.repositories.is_empty() && self.twig.is_empty() && self.associations.is_empty()
    }
}

/// A route a `#[Route(…, name: 'blog_show')]` attribute names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteAttribute {
    /// The full name (class prefix included).
    pub name: String,
    /// The full path (class prefix included), when written as a string.
    pub path: Option<String>,
    /// The name argument's string.
    pub loc: Loc,
}

/// What a [`TwigName`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TwigNameKind {
    Filter,
    Function,
    Test,
}

/// A filter, function or test a Twig extension adds: `new TwigFilter('price', …)`
/// (and `TwigFunction`, `TwigTest`), or a method with `#[AsTwigFilter('price')]`
/// (and `AsTwigFunction`, `AsTwigTest`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TwigName {
    pub kind: TwigNameKind,
    pub name: String,
    /// The name's string.
    pub loc: Loc,
}

/// An association of a Doctrine entity: a property with `#[ORM\OneToMany]`,
/// `#[ORM\ManyToMany]`, `#[ORM\ManyToOne]` or `#[ORM\OneToOne]` naming its
/// `targetEntity`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Association {
    /// FQN, no leading `\`.
    pub entity: String,
    /// Without the `$`.
    pub property: String,
    /// FQN, no leading `\`.
    pub target: String,
    /// To many (a collection of the target), not to one.
    pub many: bool,
}

/// A Doctrine repository and its entity: from the entity's
/// `#[ORM\Entity(repositoryClass: …)]`, or from the repository's
/// `parent::__construct($registry, Product::class)`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryEntity {
    /// FQN, no leading `\`.
    pub repository: String,
    /// FQN, no leading `\`.
    pub entity: String,
}
