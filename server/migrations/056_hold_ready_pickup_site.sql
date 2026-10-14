-- Add {{pickup_site}} to the shipped hold-ready notices (#145).
-- Rows an administrator has edited (name, subject, or body) are left unchanged.
-- German and Spanish notices ship as on-disk templates. This table only stores
-- english and french.

UPDATE email_templates
SET
    name = 'Réservation disponible',
    subject = 'Votre réservation est disponible',
    body_plain = $new_fr_plain$Bonjour {{firstname}} {{lastname}},

Le document que vous avez réservé vous attend à {{pickup_site}}, à retirer jusqu'au {{expires_at}} inclus.

{{title}}
{{barcode_line}}

Cordialement,
L'équipe de la bibliothèque$new_fr_plain$,
    body_html = $new_fr_html$<html><body style="font-family: Arial, sans-serif; color: #333;">
<p>Bonjour <strong>{{firstname}} {{lastname}}</strong>,</p>
<p>Le document que vous avez réservé vous attend à <strong>{{pickup_site}}</strong>, à retirer jusqu'au <strong>{{expires_at}}</strong> inclus.</p>
<p><strong>{{title}}</strong><br>{{barcode_line_html}}</p>
<p>Cordialement,<br><em>L'équipe de la bibliothèque</em></p>
</body></html>$new_fr_html$
WHERE template_id = 'hold_ready'
  AND language = 'french'
  AND name = 'Réservation disponible'
  AND subject = 'Votre réservation est disponible'
  AND body_plain = $old_fr_plain$Bonjour {{firstname}} {{lastname}},

Le document que vous avez réservé vous attend :

{{title}}
{{barcode_line}}

À retirer jusqu'au {{expires_at}} inclus.

Cordialement,
L'équipe de la bibliothèque$old_fr_plain$
  AND body_html = $old_fr_html$<html><body style="font-family: Arial, sans-serif; color: #333;">
<p>Bonjour <strong>{{firstname}} {{lastname}}</strong>,</p>
<p>Le document que vous avez réservé vous attend :</p>
<p><strong>{{title}}</strong><br>{{barcode_line_html}}</p>
<p>À retirer jusqu'au <strong>{{expires_at}}</strong> inclus.</p>
<p>Cordialement,<br><em>L'équipe de la bibliothèque</em></p>
</body></html>$old_fr_html$;

UPDATE email_templates
SET
    name = 'Hold ready for pickup',
    subject = 'Your library hold is ready for pickup',
    body_plain = $new_en_plain$Dear {{firstname}} {{lastname}},

The item you reserved is waiting for you at {{pickup_site}}. Please pick it up by {{expires_at}}.

{{title}}
{{barcode_line}}

Kind regards,
The library team$new_en_plain$,
    body_html = $new_en_html$<html><body style="font-family: Arial, sans-serif; color: #333;">
<p>Dear <strong>{{firstname}} {{lastname}}</strong>,</p>
<p>The item you reserved is waiting for you at <strong>{{pickup_site}}</strong>. Please pick it up by <strong>{{expires_at}}</strong>.</p>
<p><strong>{{title}}</strong><br>{{barcode_line_html}}</p>
<p>Kind regards,<br><em>The library team</em></p>
</body></html>$new_en_html$
WHERE template_id = 'hold_ready'
  AND language = 'english'
  AND name = 'Hold ready for pickup'
  AND subject = 'Your library hold is ready for pickup'
  AND body_plain = $old_en_plain$Dear {{firstname}} {{lastname}},

The following item you placed on hold is now available:

{{title}}
{{barcode_line}}

Please pick it up by {{expires_at}}.

Kind regards,
The library team$old_en_plain$
  AND body_html = $old_en_html$<html><body style="font-family: Arial, sans-serif; color: #333;">
<p>Dear <strong>{{firstname}} {{lastname}}</strong>,</p>
<p>The following item on <strong>hold</strong> is now available for pickup:</p>
<p><strong>{{title}}</strong><br>{{barcode_line_html}}</p>
<p>Please pick it up by <strong>{{expires_at}}</strong>.</p>
<p>Kind regards,<br><em>The library team</em></p>
</body></html>$old_en_html$;
