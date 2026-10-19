<?php
$isNew = $template === null;
$vars = implode("\n", array_map(static fn ($v) => $v['name'] . ' | ' . (is_scalar($v['sample'] ?? null) ? $v['sample'] : '') . (!empty($v['required']) ? ' | required' : ''), $version['variables'] ?? []));
$readonly = !can('templates:write');
?>
<?= $view->insert('partials/page-head', [
    'heading' => $isNew ? 'New template' : $template['name'],
    'lede' => $isNew ? 'Write the subject and body; the preview updates as you type.' : 'Slug ' . $template['slug'] . ' · editing version ' . ($version['version'] ?? '') . ' · active v' . ($template['active_version'] ?? '—'),
    'actions' => !$isNew && can('templates:write') ? '<form method="post" action="/console/templates/' . e($template['id']) . '/status">' . csrf_field() . '<input type="hidden" name="status" value="' . ($template['status'] === 'archived' ? 'active' : 'archived') . '"><button class="btn" type="submit">' . ($template['status'] === 'archived' ? 'Restore' : 'Archive') . '</button></form>' : '',
]) ?>
<div class="grid grid-2" style="align-items:start">
  <form method="post" action="<?= $isNew ? '/console/templates' : '/console/templates/' . e($template['id']) ?>" class="card card-body" data-template-editor data-preview-url="/console/templates/preview">
    <?= csrf_field() ?>
    <div class="grid grid-2">
      <?= $view->insert('partials/field', ['name' => 'name', 'label' => 'Name', 'required' => true, 'default' => $template['name'] ?? '', 'placeholder' => 'Invoice issued']) ?>
      <?= $view->insert('partials/field', ['name' => 'description', 'label' => 'Description', 'default' => $template['description'] ?? '']) ?>
    </div>
    <?= $view->insert('partials/field', ['name' => 'subject', 'label' => 'Subject', 'required' => true, 'default' => $version['subject'] ?? '']) ?>
    <?= $view->insert('partials/field', ['name' => 'html_body', 'label' => 'HTML body', 'type' => 'textarea', 'rows' => 14, 'default' => $version['html_body'] ?? '', 'attrs' => ' spellcheck="false" style="font-family:var(--mono);font-size:.82rem"']) ?>
    <?= $view->insert('partials/field', ['name' => 'text_body', 'label' => 'Plain-text body', 'type' => 'textarea', 'rows' => 6, 'default' => $version['text_body'] ?? '', 'hint' => 'Recommended: improves deliverability and accessibility.']) ?>
    <?= $view->insert('partials/field', ['name' => 'variables', 'label' => 'Variables', 'type' => 'textarea', 'rows' => 4, 'default' => $vars, 'hint' => 'One per line: name | sample value | required', 'attrs' => ' spellcheck="false" style="font-family:var(--mono);font-size:.82rem"']) ?>
    <?php if (!$readonly): ?>
      <div class="form-actions">
        <?php if ($isNew): ?><button class="btn btn-primary" type="submit">Create template</button>
        <?php else: ?><button class="btn btn-primary" type="submit" name="activate" value="1">Save and activate</button><button class="btn" type="submit">Save as draft version</button><?php endif; ?>
      </div>
    <?php endif; ?>
  </form>
  <div class="stack" style="position:sticky;top:1rem">
    <section class="card">
      <div class="card-head"><h2>Preview</h2><div class="row"><button class="btn btn-sm" type="button" data-preview-width="desktop" aria-pressed="true"><?= icon('laptop') ?><span class="sr-only">Desktop</span></button><button class="btn btn-sm" type="button" data-preview-width="mobile" aria-pressed="false"><?= icon('smartphone-check') ?><span class="sr-only">Mobile</span></button></div></div>
      <div class="card-body">
        <p class="alert alert-error" id="preview-error" hidden></p>
        <p class="small"><strong>Subject:</strong> <span id="preview-subject"></span></p>
        <iframe id="preview-frame" class="preview-frame" title="HTML preview" sandbox=""></iframe>
        <details class="mt-2"><summary class="small">Plain-text version</summary><pre id="preview-text" class="small"></pre></details>
      </div>
    </section>
    <?php if ($versions !== []): ?>
    <section class="card">
      <div class="card-head"><h2>Versions</h2></div>
      <div class="table-wrap"><table class="table table-compact"><tbody>
        <?php foreach ($versions as $v): ?>
          <tr><td><a href="?version=<?= e($v['version']) ?>">v<?= e($v['version']) ?></a><?= (int) $v['version'] === (int) ($template['active_version'] ?? 0) ? ' ' . badge('active', 'Active') : '' ?></td><td class="small truncate"><?= e($v['subject']) ?></td><td class="small muted nowrap"><?= time_tag($v['created_at']) ?></td>
          <td><?php if (can('templates:write') && (int) $v['version'] !== (int) ($template['active_version'] ?? 0)): ?><form method="post" action="/console/templates/<?= e($template['id']) ?>/activate"><?= csrf_field() ?><input type="hidden" name="version" value="<?= e($v['version']) ?>"><button class="btn btn-sm" type="submit">Activate</button></form><?php endif; ?></td></tr>
        <?php endforeach; ?>
      </tbody></table></div>
    </section>
    <?php endif; ?>
  </div>
</div>
