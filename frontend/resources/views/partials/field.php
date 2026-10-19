<?php
/** Form field: name, label, type, value, hint, required, attrs, autocomplete, placeholder, options (select), rows (textarea). */
$type = $type ?? 'text';
$id = $id ?? 'f-' . trim(str_replace(['[', ']'], '-', $name), '-');
$value = $value ?? old($name, $default ?? '');
$req = !empty($required) ? ' required' : '';
$extra = ($attrs ?? '') . (isset($autocomplete) ? ' autocomplete="' . e($autocomplete) . '"' : '') . (isset($placeholder) ? ' placeholder="' . e($placeholder) . '"' : '');
?>
<div class="field">
  <label for="<?= e($id) ?>"><?= e($label) ?><?= empty($required) && empty($noOptional) ? ' <span class="muted small">(optional)</span>' : '' ?></label>
  <?php if ($type === 'select'): ?>
    <select class="input" id="<?= e($id) ?>" name="<?= e($name) ?>"<?= $req . $extra . error_attrs($name) ?>>
      <?php foreach ($options as $k => $v): ?><option value="<?= e($k) ?>"<?= selected((string) $value, (string) $k) ?>><?= e($v) ?></option><?php endforeach; ?>
    </select>
  <?php elseif ($type === 'textarea'): ?>
    <textarea class="input" id="<?= e($id) ?>" name="<?= e($name) ?>" rows="<?= (int) ($rows ?? 4) ?>"<?= $req . $extra . error_attrs($name) ?>><?= e($value) ?></textarea>
  <?php elseif ($type === 'password'): ?>
    <div class="password-field">
      <input class="input" type="password" id="<?= e($id) ?>" name="<?= e($name) ?>"<?= $req . $extra . error_attrs($name) ?>>
      <button type="button" class="btn btn-ghost btn-icon btn-sm reveal" data-reveal="<?= e($id) ?>" aria-label="Show password"><?= icon('eye') ?></button>
    </div>
  <?php else: ?>
    <input class="input" type="<?= e($type) ?>" id="<?= e($id) ?>" name="<?= e($name) ?>" value="<?= e($value) ?>"<?= $req . $extra . error_attrs($name) ?>>
  <?php endif; ?>
  <?php if (!empty($hint)): ?><p class="hint"><?= e($hint) ?></p><?php endif; ?>
  <?= error_text($name) ?>
</div>
