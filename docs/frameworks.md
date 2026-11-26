# Frameworks

What each framework's support knows, side by side. A framework is found by a Composer
package or one of its own files (`docs/engine/frameworks.md` has the files), and the
`frameworks` setting turns each on or off (`docs/settings.md`).

The *names* are the kinds of string the support reads: in a string of that kind, completion
lists what the project declares, go to definition and hover find it, and its usages are
found and, for most, renamed with it. The *inspections* report such a name that names
nothing (`docs/inspections.md`). The types each framework adds (services, models, view
variables) and what it reads besides PHP are in its note in `docs/engine`.

This page is written by a test (`crates/phoxite/src/frameworks/overview_tests.rs`): don't
edit it by hand.

| Framework | Setting | Composer packages | Kinds of name | Inspections |
| --- | --- | --- | --- | --- |
| [Laravel](#laravel) | `frameworks.laravel` | `laravel/framework` | 24 | 15 |
| [Symfony](#symfony) | `frameworks.symfony` | `symfony/framework-bundle` | 6 | 2 |
| [WordPress](#wordpress) | `frameworks.wordpress` | `johnpbloch/wordpress`, `johnpbloch/wordpress-core`, `roots/wordpress`, `roots/wordpress-no-content`, `php-stubs/wordpress-stubs` | 13 | 1 |
| [Drupal](#drupal) | `frameworks.drupal` | `drupal/core`, `drupal/core-recommended` | 15 | 0 |
| [CodeIgniter](#codeigniter) | `frameworks.codeigniter` | `codeigniter4/framework`, `codeigniter/framework` | 18 | 15 |
| [Yii](#yii) | `frameworks.yii` | `yiisoft/yii2` | 11 | 4 |
| [CakePHP](#cakephp) | `frameworks.cakephp` | `cakephp/cakephp` | 16 | 7 |
| [Laminas](#laminas) | `frameworks.laminas` | `laminas/laminas-mvc`, `mezzio/mezzio`, `zendframework/zend-mvc`, `zendframework/zend-expressive` | 12 | 4 |
| [Magento](#magento) | `frameworks.magento` | `magento/framework`, `magento/product-community-edition`, `magento/product-enterprise-edition` | 10 | 4 |
| [Joomla](#joomla) | `frameworks.joomla` | none | 15 | 2 |

## Laravel

Names: `configKey`, `envKey`, `translationKey`, `viteAsset`, `routeName`, `controllerAction`, `resourceAction`, `middleware`, `viewName`, `validationRule`, `requestField`, `eloquentField`, `eloquentRelation`, `dbTable`, `gate`, `authGuard`, `passwordBroker`, `artisanCommand`, `artisanOption`, `livewireComponent`, `livewireParameter`, `inertiaPage`, `staticStrings`, `directoryFiles`.

Inspections: `PhpUndefinedConfigKey`, `PhpUndefinedController`, `PhpUndefinedEnvKey`, `PhpUndefinedInertiaPage`, `PhpUndefinedRoute`, `PhpUndefinedTranslation`, `PhpUndefinedView`, `PhpUndefinedViteAsset`, `PhpUnknownAbility`, `PhpUnknownArtisanCommand`, `PhpUnknownArtisanOption`, `PhpUnknownEloquentColumn`, `PhpUnknownGuard`, `PhpUnknownPasswordBroker`, `PhpUnknownValidationRule`.

More: [engine/laravel.md](engine/laravel.md).

## Symfony

Names: `symfonyRoute`, `symfonyTemplate`, `symfonyParameter`, `symfonyService`, `symfonyTranslation`, `doctrineField`.

Inspections: `PhpUndefinedRoute`, `PhpUndefinedTemplate`.

More: [engine/symfony.md](engine/symfony.md).

## WordPress

Names: `wordpressHook`, `wordpressShortcode`, `wordpressPostType`, `wordpressTaxonomy`, `wordpressScript`, `wordpressStyle`, `wordpressOption`, `wordpressImageSize`, `wordpressNavMenu`, `wordpressBlock`, `wordpressRestRoute`, `wordpressTemplatePart`, `wordpressThemeFile`.

Inspections: `PhpUndefinedHook`.

More: [engine/wordpress.md](engine/wordpress.md).

## Drupal

Names: `drupalService`, `drupalRoute`, `drupalPermission`, `drupalLibrary`, `drupalConfig`, `drupalConfigKey`, `drupalModule`, `drupalEntityType`, `drupalPlugin`, `drupalThemeHook`, `drupalMenuPath`, `drupalVariable`, `drupalTable`, `drupalColumn`, `drupalHook`.

Inspections: none.

More: [engine/drupal.md](engine/drupal.md).

## CodeIgniter

Names: `codeigniterView`, `codeigniterHelper`, `codeigniterLang`, `codeigniterRoute`, `codeigniterFilter`, `codeigniterConfig`, `codeigniterModel`, `codeigniterService`, `codeigniterController`, `codeigniterCell`, `codeigniterEvent`, `codeigniterRule`, `codeigniterRuleGroup`, `codeigniterTable`, `codeigniterColumn`, `codeigniterLibrary`, `codeigniterConfigItem`, `codeigniterUri`.

Inspections: `PhpUndefinedCodeIgniterCell`, `PhpUndefinedCodeIgniterConfig`, `PhpUndefinedCodeIgniterController`, `PhpUndefinedCodeIgniterEvent`, `PhpUndefinedCodeIgniterHelper`, `PhpUndefinedCodeIgniterLanguageLine`, `PhpUndefinedCodeIgniterModel`, `PhpUndefinedCodeIgniterRoute`, `PhpUndefinedCodeIgniterService`, `PhpUndefinedCodeIgniterView`, `PhpUnknownCodeIgniterColumn`, `PhpUnknownCodeIgniterFilter`, `PhpUnknownCodeIgniterRule`, `PhpUnknownCodeIgniterRuleGroup`, `PhpUnknownCodeIgniterTable`.

More: [engine/codeigniter.md](engine/codeigniter.md).

## Yii

Names: `yiiView`, `yiiLayout`, `yiiMail`, `yiiRoute`, `yiiComponent`, `yiiModule`, `yiiAlias`, `yiiCategory`, `yiiMessage`, `yiiAttribute`, `yiiParam`.

Inspections: `PhpUndefinedYiiLayout`, `PhpUndefinedYiiRoute`, `PhpUndefinedYiiView`, `PhpUnknownYiiAttribute`.

More: [engine/yii.md](engine/yii.md).

## CakePHP

Names: `cakeTable`, `cakeComponent`, `cakeHelper`, `cakeBehavior`, `cakeAssociation`, `cakeTemplate`, `cakeLayout`, `cakeElement`, `cakeCell`, `cakeConfig`, `cakeController`, `cakeAction`, `cakePlugin`, `cakeRoute`, `cakeMessage`, `cakeDomain`.

Inspections: `PhpUndefinedCakeAction`, `PhpUndefinedCakeBehavior`, `PhpUndefinedCakeComponent`, `PhpUndefinedCakeElement`, `PhpUndefinedCakeHelper`, `PhpUndefinedCakeLayout`, `PhpUndefinedCakeTemplate`.

More: [engine/cakephp.md](engine/cakephp.md).

## Laminas

Names: `laminasRoute`, `laminasTemplate`, `laminasService`, `laminasPlugin`, `laminasHelper`, `laminasAction`, `laminasConfig`, `laminasValidator`, `laminasFilter`, `laminasFormElement`, `laminasNavigation`, `laminasRouteParam`.

Inspections: `PhpUndefinedLaminasAction`, `PhpUndefinedLaminasRoute`, `PhpUndefinedLaminasService`, `PhpUndefinedLaminasTemplate`.

More: [engine/laminas.md](engine/laminas.md).

## Magento

Names: `magentoEvent`, `magentoConfigPath`, `magentoAcl`, `magentoTemplate`, `magentoHandle`, `magentoBlock`, `magentoModule`, `magentoRoute`, `magentoTable`, `magentoColumn`.

Inspections: `PhpUndefinedMagentoAcl`, `PhpUndefinedMagentoModule`, `PhpUndefinedMagentoPluginMethod`, `PhpUndefinedMagentoTemplate`.

More: [engine/magento.md](engine/magento.md).

## Joomla

Names: `joomlaLanguage`, `joomlaLayout`, `joomlaHtml`, `joomlaAsset`, `joomlaExtension`, `joomlaEvent`, `joomlaHandler`, `joomlaAction`, `joomlaForm`, `joomlaField`, `joomlaParam`, `joomlaTable`, `joomlaColumn`, `joomlaRoute`, `joomlaTemplate`.

Inspections: `PhpUndefinedJoomlaLanguage`, `PhpUndefinedJoomlaLayout`.

More: [engine/joomla.md](engine/joomla.md).
