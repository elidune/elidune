-- Replace the shipped « mise en attente » wording with library French.
-- Rows an administrator has edited (subject or body) are left unchanged.

UPDATE email_templates
SET
    name = 'Réservation disponible',
    subject = 'Votre réservation est disponible',
    body_plain = $plain$Bonjour {{firstname}} {{lastname}},

Le document que vous avez réservé est disponible :

{{title}}
{{barcode_line}}

Merci de le retirer avant le {{expires_at}}.

Cordialement,
L'équipe de la bibliothèque$plain$,
    body_html = $html$<html><body style="font-family: Arial, sans-serif; color: #333;">
<p>Bonjour <strong>{{firstname}} {{lastname}}</strong>,</p>
<p>Votre <strong>réservation</strong> est disponible au retrait :</p>
<p><strong>{{title}}</strong><br>{{barcode_line_html}}</p>
<p>Merci de le retirer avant le <strong>{{expires_at}}</strong>.</p>
<p>Cordialement,<br><em>L'équipe de la bibliothèque</em></p>
</body></html>$html$
WHERE template_id = 'hold_ready'
  AND language = 'french'
  AND name = 'Réservation prête au retrait'
  AND subject = 'Votre mise en attente est prête au retrait'
  AND body_plain = $old_plain$Bonjour {{firstname}} {{lastname}},

Le document suivant que vous aviez mis en attente est disponible :

{{title}}
{{barcode_line}}

Merci de le retirer avant le {{expires_at}}.

Cordialement,
L'équipe de la bibliothèque$old_plain$
  AND body_html = $old_html$<html><body style="font-family: Arial, sans-serif; color: #333;">
<p>Bonjour <strong>{{firstname}} {{lastname}}</strong>,</p>
<p>Le document suivant en <strong>mise en attente</strong> est disponible au retrait :</p>
<p><strong>{{title}}</strong><br>{{barcode_line_html}}</p>
<p>Merci de le retirer avant le <strong>{{expires_at}}</strong>.</p>
<p>Cordialement,<br><em>L'équipe de la bibliothèque</em></p>
</body></html>$old_html$;
