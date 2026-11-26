# Inspections

Every inspection the engine has, by the id the `inspections` setting takes (`phoxite.inspections`
in VS Code, `inspections` in `phoxite.json`) and a `@noinspection` comment names. Each can be
turned off or on (`enabled`) and given another `severity`; the ones marked *strict* also take
`"strict": true`, which reports what is only probably wrong.

```json
{ "inspections": { "PhpUnusedParameter": { "enabled": false }, "PhpUndefinedHook": { "enabled": true } } }
```

A framework's inspections report in projects of that framework only. This page is written
by a test (`crates/phoxite/src/inspect/reference_tests.rs`): don't edit the table by hand.

| Inspection | Severity | On by default | Strict mode |
| --- | --- | --- | --- |
| `PhpDeprecatedSyntax` | warning | yes |  |
| `PhpDeprecation` | warning | yes |  |
| `PhpFieldAssignmentTypeMismatch` | error | yes | yes |
| `PhpInaccessibleMember` | warning | yes | yes |
| `PhpIncompatibleReturnType` | error | yes | yes |
| `PhpLanguageLevel` | error | yes |  |
| `PhpMethodParametersCountMismatch` | error | yes | yes |
| `PhpMissingReturnStatement` | warning | yes |  |
| `PhpNeverFunctionReturns` | error | yes |  |
| `PhpParams` | error | yes | yes |
| `PhpReadonlyPropertyWrittenOutsideDeclarationScope` | warning | yes | yes |
| `PhpUndefinedCakeAction` | warning | yes |  |
| `PhpUndefinedCakeBehavior` | warning | yes |  |
| `PhpUndefinedCakeComponent` | warning | yes |  |
| `PhpUndefinedCakeElement` | warning | yes |  |
| `PhpUndefinedCakeHelper` | warning | yes |  |
| `PhpUndefinedCakeLayout` | warning | yes |  |
| `PhpUndefinedCakeTemplate` | warning | yes |  |
| `PhpUndefinedClass` | warning | yes |  |
| `PhpUndefinedClassConstant` | warning | yes |  |
| `PhpUndefinedCodeIgniterCell` | warning | yes |  |
| `PhpUndefinedCodeIgniterConfig` | warning | yes |  |
| `PhpUndefinedCodeIgniterController` | warning | yes |  |
| `PhpUndefinedCodeIgniterEvent` | warning | no |  |
| `PhpUndefinedCodeIgniterHelper` | warning | yes |  |
| `PhpUndefinedCodeIgniterLanguageLine` | warning | yes |  |
| `PhpUndefinedCodeIgniterModel` | warning | yes |  |
| `PhpUndefinedCodeIgniterRoute` | warning | yes |  |
| `PhpUndefinedCodeIgniterService` | warning | yes |  |
| `PhpUndefinedCodeIgniterView` | warning | yes |  |
| `PhpUndefinedConfigKey` | warning | yes |  |
| `PhpUndefinedConstant` | warning | yes |  |
| `PhpUndefinedController` | warning | yes |  |
| `PhpUndefinedDocClass` | information | yes |  |
| `PhpUndefinedEnvKey` | warning | yes |  |
| `PhpUndefinedField` | warning | yes |  |
| `PhpUndefinedFunction` | warning | yes |  |
| `PhpUndefinedHook` | warning | no |  |
| `PhpUndefinedInertiaPage` | warning | yes |  |
| `PhpUndefinedJoomlaLanguage` | warning | yes |  |
| `PhpUndefinedJoomlaLayout` | warning | yes |  |
| `PhpUndefinedLaminasAction` | warning | yes |  |
| `PhpUndefinedLaminasRoute` | warning | yes |  |
| `PhpUndefinedLaminasService` | warning | yes |  |
| `PhpUndefinedLaminasTemplate` | warning | yes |  |
| `PhpUndefinedMagentoAcl` | warning | yes |  |
| `PhpUndefinedMagentoModule` | warning | yes |  |
| `PhpUndefinedMagentoPluginMethod` | warning | yes |  |
| `PhpUndefinedMagentoTemplate` | warning | yes |  |
| `PhpUndefinedMethod` | warning | yes |  |
| `PhpUndefinedRoute` | warning | yes |  |
| `PhpUndefinedTemplate` | warning | yes |  |
| `PhpUndefinedTranslation` | warning | yes |  |
| `PhpUndefinedVariable` | warning | yes |  |
| `PhpUndefinedView` | warning | yes |  |
| `PhpUndefinedViteAsset` | warning | yes |  |
| `PhpUndefinedYiiLayout` | warning | yes |  |
| `PhpUndefinedYiiRoute` | warning | yes |  |
| `PhpUndefinedYiiView` | warning | yes |  |
| `PhpUnknownAbility` | warning | yes |  |
| `PhpUnknownArtisanCommand` | warning | yes |  |
| `PhpUnknownArtisanOption` | warning | yes |  |
| `PhpUnknownCodeIgniterColumn` | warning | no |  |
| `PhpUnknownCodeIgniterFilter` | warning | yes |  |
| `PhpUnknownCodeIgniterRule` | warning | yes |  |
| `PhpUnknownCodeIgniterRuleGroup` | warning | yes |  |
| `PhpUnknownCodeIgniterTable` | warning | no |  |
| `PhpUnknownEloquentColumn` | warning | yes |  |
| `PhpUnknownGuard` | warning | yes |  |
| `PhpUnknownNamedArgument` | error | yes | yes |
| `PhpUnknownPasswordBroker` | warning | yes |  |
| `PhpUnknownValidationRule` | warning | yes |  |
| `PhpUnknownYiiAttribute` | warning | yes |  |
| `PhpUnreachableStatement` | warning | yes |  |
| `PhpUnusedAlias` | warning | yes |  |
| `PhpUnusedLocalVariable` | warning | yes |  |
| `PhpUnusedParameter` | warning | yes |  |
| `PhpUnusedPrivateConstant` | warning | yes |  |
| `PhpUnusedPrivateField` | warning | yes |  |
| `PhpUnusedPrivateMethod` | warning | yes |  |
| `PhpVoidFunctionReturnsValue` | error | yes |  |
