<?php

/*
 * The hooks the fixture's module implements, documented as Drupal's `*.api.php` files
 * document theirs. Written for this fixture.
 */

use Drupal\Core\Entity\EntityInterface;
use Drupal\Core\Form\FormStateInterface;

/**
 * Acts on an entity before it is saved.
 *
 * @param \Drupal\Core\Entity\EntityInterface $entity
 *   The entity.
 */
function hook_entity_presave(EntityInterface $entity): void
{
}

/**
 * Acts on an entity of one type before it is saved.
 *
 * @param \Drupal\Core\Entity\EntityInterface $entity
 *   The entity.
 */
function hook_ENTITY_TYPE_presave(EntityInterface $entity): void
{
}

/**
 * Changes a form.
 *
 * @param array<string, mixed> $form
 *   The form.
 */
function hook_form_alter(array &$form, FormStateInterface $form_state, string $form_id): void
{
}

/**
 * Says what the module themes.
 *
 * @return array<string, array<string, mixed>>
 *   The theme hooks.
 */
function hook_theme(): array
{
    return [];
}

/**
 * Runs on a schedule.
 */
function hook_cron(): void
{
}
