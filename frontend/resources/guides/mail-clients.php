<?php

declare(strict_types=1);

/*
 * Mail-app setup guides.
 *
 * Reviewed against vendor documentation on 13 October 2026 (see `sources`).
 * Menus change between app versions, so every step explains *why* it is
 * needed, not only which button to press, and has a "cannot find it" hint.
 *
 * Placeholders, filled from the platform's validated settings:
 *   {address} {org} {imap_host} {imap_port} {smtp_host} {smtp_port}
 *   {smtp_security} {smtp_alt} {webmail_url}
 *
 * `fields` lists the settings shown with copy buttons in that step.
 * `mock` drives the illustrative phone diagram (not a screenshot).
 */

return [
    'reviewed' => '2026-10-13',

    'devices' => [
        'android' => ['label' => 'Android phone or tablet', 'icon' => 'android', 'hint' => 'Samsung, Tecno, Infinix, Xiaomi, Pixel and other Android devices'],
        'ios' => ['label' => 'iPhone or iPad', 'icon' => 'apple', 'hint' => 'Any iPhone or iPad'],
        'computer' => ['label' => 'Computer or other device', 'icon' => 'laptop', 'hint' => 'Windows, Mac, Linux, or a tablet with another email app'],
    ],

    'apps' => [
        'gmail' => ['label' => 'Gmail', 'icon' => 'mail', 'hint' => 'The red-and-white Gmail app. You can keep your personal Gmail and add work email next to it.'],
        'apple_mail' => ['label' => 'Apple Mail', 'icon' => 'apple', 'hint' => 'The Mail app that comes with your iPhone or iPad.'],
        'outlook' => ['label' => 'Microsoft Outlook', 'icon' => 'mail', 'hint' => 'The blue Outlook app.'],
        'other' => ['label' => 'Another email app', 'icon' => 'grid', 'hint' => 'Samsung Email, Thunderbird, Spark, BlueMail or any app that supports IMAP.'],
        'webmail' => ['label' => 'Use the browser instead', 'icon' => 'globe', 'hint' => 'No app needed: read your work email in any web browser.'],
    ],

    /* Which apps to offer per device, in order. */
    'device_apps' => [
        'android' => ['gmail', 'outlook', 'other', 'webmail'],
        'ios' => ['apple_mail', 'gmail', 'outlook', 'other', 'webmail'],
        'computer' => ['outlook', 'other', 'webmail'],
    ],

    'guides' => [
        'android-gmail' => [
            'device' => 'android',
            'app' => 'gmail',
            'title' => 'Add your work email to Gmail on Android',
            'time' => 'About 5 minutes',
            'sources' => [
                ['label' => 'Google: Add another email account to Gmail', 'url' => 'https://support.google.com/mail/answer/6078445?co=GENIE.Platform%3DAndroid'],
            ],
            'steps' => [
                [
                    'title' => 'Open Gmail and tap your profile picture',
                    'body' => 'Open the Gmail app. At the top right, tap your profile picture or initial.',
                    'why' => 'Gmail keeps every account under your picture. Adding work email here does not remove or change your personal Gmail.',
                    'cant_find' => 'If you do not see a picture, tap the menu (three lines, top left), scroll to the bottom and tap Settings, then Add account.',
                    'mock' => ['title' => 'Gmail', 'kind' => 'inbox', 'highlight' => 'avatar'],
                ],
                [
                    'title' => 'Tap "Add another account"',
                    'body' => 'In the list that opens, tap Add another account.',
                    'why' => 'This starts adding a second account next to the ones you already have.',
                    'cant_find' => 'Some versions say "Add account". It is the same option.',
                    'mock' => ['title' => 'Accounts', 'kind' => 'list', 'rows' => ['personal@gmail.com', 'Add another account', 'Manage accounts on this device'], 'highlight' => 1],
                ],
                [
                    'title' => 'Choose "Other"',
                    'body' => 'From the list of email providers, choose Other. Do not choose Google.',
                    'why' => 'Your work email is hosted by {org}, not by Google, so it is not a Google account.',
                    'cant_find' => 'Scroll down: Other is usually the last item in the list.',
                    'mock' => ['title' => 'Set up email', 'kind' => 'list', 'rows' => ['Google', 'Outlook, Hotmail and Live', 'Yahoo', 'Exchange and Office 365', 'Other'], 'highlight' => 4],
                ],
                [
                    'title' => 'Type your full work email address',
                    'body' => 'Type {address}, then tap Manual setup if you see it (otherwise tap Next).',
                    'why' => 'Manual setup lets you enter your institution\'s exact mail server, which works on every version of Gmail.',
                    'cant_find' => 'If Gmail goes straight to a password screen, that is fine: continue with the next steps and check the settings when Gmail shows them.',
                    'fields' => ['username'],
                    'mock' => ['title' => 'Add your email address', 'kind' => 'form', 'rows' => ['{address}'], 'buttons' => ['Manual setup', 'Next'], 'highlight' => 'Manual setup'],
                ],
                [
                    'title' => 'Choose "Personal (IMAP)"',
                    'body' => 'When Gmail asks what type of account this is, choose Personal (IMAP).',
                    'why' => 'IMAP keeps your email in sync: messages you read or delete on your phone are also read or deleted in webmail and on other devices.',
                    'cant_find' => 'Do not choose POP3 or Exchange. If only "IMAP" is shown, choose it.',
                    'mock' => ['title' => 'What type of account is this?', 'kind' => 'list', 'rows' => ['Personal (POP3)', 'Personal (IMAP)', 'Exchange'], 'highlight' => 1],
                ],
                [
                    'title' => 'Enter your mailbox password',
                    'body' => 'Type the password you chose for your work email, then tap Next.',
                    'why' => 'Gmail stores the password securely on your phone. Your IT team never sees or knows it.',
                    'cant_find' => 'Forgot it? Use "Forgot password" on the work email sign-in page, or ask your IT team to send you a reset link.',
                    'mock' => ['title' => '{address}', 'kind' => 'form', 'rows' => ['Password  ••••••••••'], 'buttons' => ['Next'], 'highlight' => 'Next'],
                ],
                [
                    'title' => 'Check the incoming server settings',
                    'body' => 'Make sure the incoming settings match exactly, then tap Next.',
                    'why' => 'These tell Gmail where your work email is kept. SSL/TLS keeps your password and messages encrypted.',
                    'cant_find' => 'If Gmail shows a different server, replace it with the one here. Never choose "None" as the security type.',
                    'fields' => ['username', 'imap_host', 'imap_port', 'imap_security'],
                    'mock' => ['title' => 'Incoming server settings', 'kind' => 'form', 'rows' => ['Username  {address}', 'Server  {imap_host}', 'Port  {imap_port}', 'Security type  SSL/TLS'], 'buttons' => ['Next'], 'highlight' => 'Next'],
                ],
                [
                    'title' => 'Check the outgoing server settings',
                    'body' => 'Keep "Require sign-in" turned on, check the settings, then tap Next.',
                    'why' => 'The outgoing server sends your messages. Signing in proves the messages really come from you.',
                    'cant_find' => 'If sending fails later with this port, use the alternative shown in Advanced settings ({smtp_alt}).',
                    'fields' => ['username', 'smtp_host', 'smtp_port', 'smtp_security'],
                    'mock' => ['title' => 'Outgoing server settings', 'kind' => 'form', 'rows' => ['Require sign-in  ●', 'Username  {address}', 'SMTP server  {smtp_host}', 'Port  {smtp_port}', 'Security type  {smtp_security}'], 'buttons' => ['Next'], 'highlight' => 'Next'],
                ],
                [
                    'title' => 'Finish: name the account',
                    'body' => 'Choose how often to check for mail (every 15 minutes is a good default), tap Next, give the account a name such as "Work", type your name as it should appear on emails, and tap Next.',
                    'why' => 'The account name only helps you tell personal and work mail apart on this phone.',
                    'cant_find' => 'All these options can be changed later in Gmail settings.',
                    'mock' => ['title' => 'Your account is set up', 'kind' => 'form', 'rows' => ['Account name  Work', 'Your name  shown on sent emails'], 'buttons' => ['Next'], 'highlight' => 'Next'],
                ],
            ],
        ],

        'ios-gmail' => [
            'device' => 'ios',
            'app' => 'gmail',
            'title' => 'Add your work email to Gmail on iPhone or iPad',
            'time' => 'About 5 minutes',
            'sources' => [
                ['label' => 'Google: Add another email account to Gmail (iPhone & iPad)', 'url' => 'https://support.google.com/mail/answer/6078445?co=GENIE.Platform%3DiOS'],
            ],
            'steps' => [
                [
                    'title' => 'Open Gmail and tap your profile picture',
                    'body' => 'Open the Gmail app and tap your profile picture at the top right, in the search bar.',
                    'why' => 'Your accounts are listed under your picture. Adding work email keeps your personal account as it is.',
                    'cant_find' => 'If you have never signed in to Gmail on this device, the app asks you to add an account straight away; continue with the next step.',
                    'mock' => ['title' => 'Gmail', 'kind' => 'inbox', 'highlight' => 'avatar'],
                ],
                [
                    'title' => 'Tap "Add another account"',
                    'body' => 'Tap Add another account.',
                    'why' => 'This adds work email as a second account.',
                    'cant_find' => 'It is below the list of accounts already on the phone.',
                    'mock' => ['title' => 'Accounts', 'kind' => 'list', 'rows' => ['personal@gmail.com', 'Add another account', 'Manage accounts on this device'], 'highlight' => 1],
                ],
                [
                    'title' => 'Choose "Other (IMAP)"',
                    'body' => 'From the list of providers, choose Other (IMAP).',
                    'why' => 'Your work email is hosted by {org}. IMAP is the standard way email apps connect to it and keeps every device in sync.',
                    'cant_find' => 'It is the last option, under Google, iCloud, Outlook and Yahoo.',
                    'mock' => ['title' => 'Add account', 'kind' => 'list', 'rows' => ['Google', 'iCloud', 'Outlook, Hotmail and Live', 'Yahoo', 'Other (IMAP)'], 'highlight' => 4],
                ],
                [
                    'title' => 'Enter your work email address',
                    'body' => 'Type {address} and tap Next.',
                    'why' => 'Your full address is also your user name for the mail server.',
                    'cant_find' => 'Type the whole address, including the part after @.',
                    'fields' => ['username'],
                    'mock' => ['title' => 'Add your email address', 'kind' => 'form', 'rows' => ['{address}'], 'buttons' => ['Next'], 'highlight' => 'Next'],
                ],
                [
                    'title' => 'Enter the incoming (IMAP) settings',
                    'body' => 'Enter the host name, your full email address as the user name, and your mailbox password. Make sure the security type is SSL/TLS and the port is {imap_port}. Tap Next.',
                    'why' => 'This connects Gmail to the server that stores your work email, over an encrypted connection.',
                    'cant_find' => 'Port and security may be under "Advanced". Never choose "None" as the security type.',
                    'fields' => ['imap_host', 'username', 'imap_port', 'imap_security'],
                    'mock' => ['title' => 'IMAP settings', 'kind' => 'form', 'rows' => ['Hostname  {imap_host}', 'Username  {address}', 'Password  ••••••••', 'Port  {imap_port}  SSL/TLS'], 'buttons' => ['Next'], 'highlight' => 'Next'],
                ],
                [
                    'title' => 'Enter the outgoing (SMTP) settings',
                    'body' => 'Enter the outgoing host name, your full email address and your mailbox password. Check the port and security, then tap Next.',
                    'why' => 'The outgoing server sends mail on your behalf after checking your password.',
                    'cant_find' => 'If sending fails with this port, try the alternative from Advanced settings ({smtp_alt}).',
                    'fields' => ['smtp_host', 'username', 'smtp_port', 'smtp_security'],
                    'mock' => ['title' => 'SMTP settings', 'kind' => 'form', 'rows' => ['Hostname  {smtp_host}', 'Username  {address}', 'Password  ••••••••', 'Port  {smtp_port}  {smtp_security}'], 'buttons' => ['Next'], 'highlight' => 'Next'],
                ],
                [
                    'title' => 'Finish',
                    'body' => 'Gmail checks the settings and opens your work inbox. You can switch between personal and work email by tapping your profile picture.',
                    'why' => 'Each account keeps its own inbox, so work and personal mail never mix.',
                    'cant_find' => 'If Gmail shows an error, open the help below and choose the message you see.',
                    'mock' => ['title' => 'Work inbox', 'kind' => 'inbox', 'highlight' => 'none'],
                ],
            ],
        ],

        'ios-apple_mail' => [
            'device' => 'ios',
            'app' => 'apple_mail',
            'title' => 'Add your work email to the Mail app on iPhone or iPad',
            'time' => 'About 4 minutes',
            'profile' => true,
            'sources' => [
                ['label' => 'Apple: Add an email account to your iPhone or iPad', 'url' => 'https://support.apple.com/en-us/102619'],
            ],
            'steps' => [
                [
                    'title' => 'Open Settings, then Mail',
                    'body' => 'Open the Settings app. Tap Apps, then Mail, then Mail Accounts.',
                    'why' => 'Email accounts for the built-in Mail app are added in Settings, not inside Mail itself.',
                    'cant_find' => 'On iOS 17 and earlier the path is Settings → Mail → Accounts. Older versions call it "Passwords & Accounts".',
                    'mock' => ['title' => 'Settings', 'kind' => 'list', 'rows' => ['Apps', 'Mail', 'Mail Accounts'], 'highlight' => 2],
                ],
                [
                    'title' => 'Tap "Add Account"',
                    'body' => 'Tap Add Account. If you are asked for your email address, type {address} and tap Next.',
                    'why' => 'This starts adding a new account; existing accounts are not changed.',
                    'cant_find' => 'Add Account is below your existing accounts.',
                    'fields' => ['username'],
                    'mock' => ['title' => 'Mail Accounts', 'kind' => 'list', 'rows' => ['iCloud', 'Add Account'], 'highlight' => 1],
                ],
                [
                    'title' => 'Choose "Add Other Account", then "Mail Account"',
                    'body' => 'Tap Add Other Account, then Mail Account.',
                    'why' => 'Your work email is not iCloud, Google or Microsoft, so it is added as an "other" mail account.',
                    'cant_find' => 'On older versions tap Other at the bottom of the provider list, then Add Mail Account.',
                    'mock' => ['title' => 'Add Account', 'kind' => 'list', 'rows' => ['iCloud', 'Microsoft Exchange', 'Google', 'Yahoo!', 'Add Other Account'], 'highlight' => 4],
                ],
                [
                    'title' => 'Enter your name, email and password',
                    'body' => 'Name: your name as it should appear on emails. Email: {address}. Password: your mailbox password. Description: for example "Work". Tap Next.',
                    'why' => 'The password stays on your iPhone. Your IT team never sees it.',
                    'cant_find' => 'Description is only a label so you can tell accounts apart.',
                    'fields' => ['username'],
                    'mock' => ['title' => 'New Account', 'kind' => 'form', 'rows' => ['Name  Your name', 'Email  {address}', 'Password  ••••••••', 'Description  Work'], 'buttons' => ['Next'], 'highlight' => 'Next'],
                ],
                [
                    'title' => 'Choose IMAP and enter the server names',
                    'body' => 'Keep IMAP selected. Under Incoming Mail Server enter the host name, your full email address as the user name and your password. Under Outgoing Mail Server enter the outgoing host name, your full email address and your password. Tap Next.',
                    'why' => 'IMAP keeps your iPhone, webmail and other devices in sync. Mail turns on encryption automatically for these servers.',
                    'cant_find' => 'If Mail found the settings by itself and shows "Save", you can skip ahead.',
                    'fields' => ['imap_host', 'smtp_host', 'username'],
                    'mock' => ['title' => 'New Account', 'kind' => 'form', 'rows' => ['IMAP  |  POP', 'Incoming  {imap_host}', 'Outgoing  {smtp_host}', 'User Name  {address}'], 'buttons' => ['Next'], 'highlight' => 'Next'],
                ],
                [
                    'title' => 'Save',
                    'body' => 'Mail checks the settings. Keep Mail turned on and tap Save.',
                    'why' => 'Your work inbox now appears in the Mail app under Mailboxes.',
                    'cant_find' => 'If Mail says it "cannot verify server identity", tap Cancel, do not tap Continue, and open the help for certificate warnings below.',
                    'mock' => ['title' => 'IMAP', 'kind' => 'list', 'rows' => ['Mail  ●', 'Notes  ○'], 'highlight' => 0],
                ],
                [
                    'title' => 'Optional: confirm encryption and ports',
                    'body' => 'In Settings → Apps → Mail → Mail Accounts → your work account → Account → Advanced, check that "Use SSL" is on and the server port is {imap_port}. Under Outgoing Mail Server → SMTP → the server, check port {smtp_port} and "Use SSL".',
                    'why' => 'This is only needed if mail does not arrive or send. It confirms the connection is encrypted.',
                    'cant_find' => 'Advanced is at the bottom of the account screen.',
                    'fields' => ['imap_port', 'smtp_port', 'smtp_security'],
                    'mock' => ['title' => 'Advanced', 'kind' => 'form', 'rows' => ['Use SSL  ●', 'Server Port  {imap_port}'], 'buttons' => [], 'highlight' => 'none'],
                ],
            ],
        ],

        'android-outlook' => [
            'device' => 'android',
            'app' => 'outlook',
            'title' => 'Add your work email to Outlook on Android',
            'time' => 'About 5 minutes',
            'sources' => [
                ['label' => 'Microsoft: Set up Outlook for iOS and Android', 'url' => 'https://support.microsoft.com/en-us/office/setup-outlook-for-ios-and-android-5a8bb76e-9bcb-4cf0-ae94-bda414104d16'],
                ['label' => 'Microsoft: Set up an IMAP account', 'url' => 'https://support.microsoft.com/en-us/outlook/how-do-i-set-up-an-imap-account'],
            ],
            'steps' => [
                [
                    'title' => 'Open Outlook and add an account',
                    'body' => 'Open Outlook. If this is your first account, tap Add account. Otherwise tap your profile icon (top left), then Settings (the gear), then Add account → Add email account.',
                    'why' => 'Outlook can hold several accounts; adding work email keeps your other accounts.',
                    'cant_find' => 'In some versions the button is a "+" next to your account icons.',
                    'mock' => ['title' => 'Settings', 'kind' => 'list', 'rows' => ['Accounts', 'Add email account', 'Notifications'], 'highlight' => 1],
                ],
                [
                    'title' => 'Type your work email address',
                    'body' => 'Type {address} and tap Continue.',
                    'why' => 'Outlook first tries to find the settings by itself.',
                    'cant_find' => 'If Outlook opens a Microsoft or Google sign-in page, go back: your work email uses IMAP, set up in the next step.',
                    'fields' => ['username'],
                    'mock' => ['title' => 'Add account', 'kind' => 'form', 'rows' => ['{address}'], 'buttons' => ['Continue'], 'highlight' => 'Continue'],
                ],
                [
                    'title' => 'Choose IMAP',
                    'body' => 'If Outlook asks for the account type or provider, choose IMAP. If it guessed another provider, tap "Not ...?" or "Change provider" and then IMAP.',
                    'why' => 'IMAP is how Outlook connects to mail hosted by {org}.',
                    'cant_find' => 'Look for "Setup account manually" or "Choose your account type".',
                    'mock' => ['title' => 'Choose provider', 'kind' => 'list', 'rows' => ['Office 365', 'Exchange', 'Outlook.com', 'Google', 'IMAP'], 'highlight' => 4],
                ],
                [
                    'title' => 'Turn on advanced settings and enter the incoming server',
                    'body' => 'Enter your name, email address and mailbox password. Turn on Advanced settings. Under IMAP incoming mail server enter the host name and port {imap_port}, choose SSL/TLS, and use your full email address as the user name.',
                    'why' => 'This tells Outlook exactly where your work mail is stored, using an encrypted connection.',
                    'cant_find' => 'The switch may be called "Use advanced settings". Never choose "None" for security.',
                    'fields' => ['imap_host', 'imap_port', 'imap_security', 'username'],
                    'mock' => ['title' => 'IMAP', 'kind' => 'form', 'rows' => ['Advanced settings  ●', 'IMAP server  {imap_host}', 'Port  {imap_port}', 'Security  SSL/TLS'], 'buttons' => [], 'highlight' => 'none'],
                ],
                [
                    'title' => 'Enter the outgoing (SMTP) server and save',
                    'body' => 'Under SMTP outgoing mail server enter the host name, port {smtp_port} and security {smtp_security}, with your full email address and password. Tap the check mark (✓) to save.',
                    'why' => 'The outgoing server sends your messages after checking your password.',
                    'cant_find' => 'If sending fails with this port, use the alternative from Advanced settings ({smtp_alt}).',
                    'fields' => ['smtp_host', 'smtp_port', 'smtp_security', 'username'],
                    'mock' => ['title' => 'IMAP', 'kind' => 'form', 'rows' => ['SMTP server  {smtp_host}', 'Port  {smtp_port}', 'Security  {smtp_security}', 'Username  {address}'], 'buttons' => ['✓'], 'highlight' => '✓'],
                ],
            ],
        ],

        'ios-outlook' => [
            'device' => 'ios',
            'app' => 'outlook',
            'title' => 'Add your work email to Outlook on iPhone or iPad',
            'time' => 'About 5 minutes',
            'sources' => [
                ['label' => 'Microsoft: Set up Outlook for iOS and Android', 'url' => 'https://support.microsoft.com/en-us/office/setup-outlook-for-ios-and-android-5a8bb76e-9bcb-4cf0-ae94-bda414104d16'],
                ['label' => 'Microsoft: Set up an IMAP account', 'url' => 'https://support.microsoft.com/en-us/outlook/how-do-i-set-up-an-imap-account'],
            ],
            'steps' => [
                [
                    'title' => 'Open Outlook and add an email account',
                    'body' => 'Open Outlook. Tap your profile icon at the top left, then the gear (Settings), then Add Mail Account → Add Email Account.',
                    'why' => 'This adds work email next to any accounts you already use in Outlook.',
                    'cant_find' => 'On first launch, Outlook shows "Add account" right away.',
                    'mock' => ['title' => 'Settings', 'kind' => 'list', 'rows' => ['Mail accounts', 'Add Mail Account', 'Notifications'], 'highlight' => 1],
                ],
                [
                    'title' => 'Type your work email address',
                    'body' => 'Type {address} and tap Add Account.',
                    'why' => 'Outlook first tries to detect your provider.',
                    'cant_find' => 'If Outlook offers to sign in with Microsoft or Google, choose the option to set up the account manually instead.',
                    'fields' => ['username'],
                    'mock' => ['title' => 'Add Email Account', 'kind' => 'form', 'rows' => ['{address}'], 'buttons' => ['Add Account'], 'highlight' => 'Add Account'],
                ],
                [
                    'title' => 'Choose IMAP',
                    'body' => 'Choose IMAP as the account type. If Outlook guessed a different provider, tap "Not ...?" at the top right and choose IMAP.',
                    'why' => 'IMAP is how Outlook connects to mail hosted by {org}.',
                    'cant_find' => 'It may be under "Setup account manually".',
                    'mock' => ['title' => 'Choose your account type', 'kind' => 'list', 'rows' => ['Office 365', 'Exchange', 'Outlook.com', 'Yahoo', 'IMAP'], 'highlight' => 4],
                ],
                [
                    'title' => 'Enter the incoming server',
                    'body' => 'Enter your email address, password and name. Turn on "Use advanced settings". Enter the IMAP host name, user name (your full address) and port {imap_port} with SSL/TLS.',
                    'why' => 'This connects Outlook to the server that keeps your work mail, using encryption.',
                    'cant_find' => 'Never choose "None" for security.',
                    'fields' => ['imap_host', 'imap_port', 'imap_security', 'username'],
                    'mock' => ['title' => 'IMAP', 'kind' => 'form', 'rows' => ['Use advanced settings  ●', 'IMAP Hostname  {imap_host}', 'Port  {imap_port}', 'Security  SSL/TLS'], 'buttons' => [], 'highlight' => 'none'],
                ],
                [
                    'title' => 'Enter the outgoing server and save',
                    'body' => 'Enter the SMTP host name, your full address and password, port {smtp_port} and security {smtp_security}. Tap the check mark (✓) at the top right.',
                    'why' => 'The outgoing server sends your messages after checking your password.',
                    'cant_find' => 'If sending fails with this port, use the alternative from Advanced settings ({smtp_alt}).',
                    'fields' => ['smtp_host', 'smtp_port', 'smtp_security', 'username'],
                    'mock' => ['title' => 'IMAP', 'kind' => 'form', 'rows' => ['SMTP Hostname  {smtp_host}', 'Port  {smtp_port}', 'Security  {smtp_security}', 'Username  {address}'], 'buttons' => ['✓'], 'highlight' => '✓'],
                ],
            ],
        ],

        'any-other' => [
            'device' => 'any',
            'app' => 'other',
            'title' => 'Add your work email to another email app',
            'time' => 'About 5 minutes',
            'sources' => [
                ['label' => 'Mozilla: Thunderbird automatic account configuration', 'url' => 'https://wiki.mozilla.org/Thunderbird:Autoconfiguration'],
                ['label' => 'RFC 8314: Use of TLS for Email Submission and Access', 'url' => 'https://www.rfc-editor.org/rfc/rfc8314.html'],
            ],
            'steps' => [
                [
                    'title' => 'Start adding a new account',
                    'body' => 'In your email app, find "Add account". Enter your name, {address} and your mailbox password.',
                    'why' => 'Apps such as Thunderbird, Samsung Email and FairEmail can often find the settings for you from your address.',
                    'cant_find' => 'It is usually in the app\'s settings or account menu.',
                    'fields' => ['username'],
                    'mock' => ['title' => 'Add account', 'kind' => 'form', 'rows' => ['Name  Your name', 'Email  {address}', 'Password  ••••••••'], 'buttons' => ['Continue'], 'highlight' => 'Continue'],
                ],
                [
                    'title' => 'If asked, choose IMAP and manual setup',
                    'body' => 'If the app asks for the account type, choose IMAP (not POP3, not Exchange). If it cannot find the settings, choose manual setup.',
                    'why' => 'IMAP keeps all your devices in sync with the server.',
                    'cant_find' => 'Manual setup is sometimes called "Configure manually" or "Advanced setup".',
                    'mock' => ['title' => 'Account type', 'kind' => 'list', 'rows' => ['IMAP', 'POP3', 'Exchange'], 'highlight' => 0],
                ],
                [
                    'title' => 'Incoming server (IMAP)',
                    'body' => 'Enter these incoming settings. The user name is your full email address.',
                    'why' => 'Implicit TLS (SSL/TLS) encrypts the connection from the first byte.',
                    'cant_find' => 'Authentication should be "Normal password". Never turn encryption off.',
                    'fields' => ['imap_host', 'imap_port', 'imap_security', 'username', 'auth'],
                    'mock' => ['title' => 'Incoming server', 'kind' => 'form', 'rows' => ['Server  {imap_host}', 'Port  {imap_port}', 'Security  SSL/TLS', 'Username  {address}'], 'buttons' => [], 'highlight' => 'none'],
                ],
                [
                    'title' => 'Outgoing server (SMTP)',
                    'body' => 'Enter these outgoing settings, with sign-in (authentication) turned on.',
                    'why' => 'Outgoing mail always requires your password, so nobody else can send as you.',
                    'cant_find' => 'Alternative if your network blocks this port: {smtp_alt}.',
                    'fields' => ['smtp_host', 'smtp_port', 'smtp_security', 'username', 'auth'],
                    'mock' => ['title' => 'Outgoing server', 'kind' => 'form', 'rows' => ['Server  {smtp_host}', 'Port  {smtp_port}', 'Security  {smtp_security}', 'Authentication  Normal password'], 'buttons' => ['Done'], 'highlight' => 'Done'],
                ],
            ],
        ],

        'any-webmail' => [
            'device' => 'any',
            'app' => 'webmail',
            'title' => 'Use work email in your browser',
            'time' => 'Under a minute',
            'sources' => [],
            'steps' => [
                [
                    'title' => 'Open webmail',
                    'body' => 'Open {webmail_url} in Chrome, Safari or any browser and sign in with {address} and your mailbox password.',
                    'why' => 'Webmail needs no setup and works on any device, including shared computers.',
                    'cant_find' => 'On your phone you can add it to the home screen from the browser\'s Share or menu button.',
                    'fields' => ['webmail', 'username'],
                    'mock' => ['title' => 'Work email', 'kind' => 'form', 'rows' => ['Email  {address}', 'Password  ••••••••'], 'buttons' => ['Sign in'], 'highlight' => 'Sign in'],
                ],
                [
                    'title' => 'Sign out on shared computers',
                    'body' => 'When you finish on a computer that others use, tap your name and choose Sign out.',
                    'why' => 'Signing out stops the next person from opening your mail.',
                    'cant_find' => 'Sign out is in the account menu at the top of webmail.',
                    'mock' => ['title' => 'Account', 'kind' => 'list', 'rows' => ['Settings', 'Connect phone', 'Sign out'], 'highlight' => 2],
                ],
            ],
        ],
    ],

    /* Troubleshooting topics: plain-language cause and safe next actions. */
    'help' => [
        'wrong-password' => [
            'title' => 'The app says my password is wrong',
            'cause' => 'The password was mistyped, it was changed recently, or the app saved an old password.',
            'steps' => [
                'Type the password again carefully. Passwords are case-sensitive.',
                'Check that the user name is your full email address ({address}), not only the part before @.',
                'Test the password by signing in to webmail at {webmail_url}. If that also fails, choose "Forgot password" there.',
                'If you changed your password recently, update it in the app\'s account settings.',
            ],
            'support' => 'If you cannot sign in to webmail either, ask your IT team to send you a password reset link. They will never ask for your current password.',
        ],
        'account-not-found' => [
            'title' => 'The app says the account or server cannot be found',
            'cause' => 'A typing mistake in the email address or server name, or the app tried to treat your work email as a Google or Microsoft account.',
            'steps' => [
                'Check the spelling of {address}.',
                'Choose manual setup with IMAP instead of letting the app sign in with Google or Microsoft.',
                'Copy the server names from this page using the copy buttons instead of typing them.',
            ],
            'support' => 'If the server names are copied exactly and it still fails, contact your IT team with the app name and the exact error message.',
        ],
        'auth-failed' => [
            'title' => 'Authentication failed or sign-in was refused',
            'cause' => 'The user name is not the full address, the password is wrong, or the mailbox is disabled or locked after several wrong passwords.',
            'steps' => [
                'Use your full email address as the user name for both incoming and outgoing servers.',
                'Make sure "Require sign-in" (authentication) is on for the outgoing server.',
                'If you entered the wrong password several times, wait 15 minutes before trying again.',
            ],
            'support' => 'Your IT team can unlock the mailbox or send you a reset link. Never share your password with anyone, including IT staff.',
        ],
        'cannot-connect' => [
            'title' => 'Cannot connect to the server',
            'cause' => 'No internet connection, a network that blocks email ports (some public Wi-Fi), or a wrong port or security setting.',
            'steps' => [
                'Check that the phone has internet: open a website in the browser.',
                'Try mobile data instead of Wi-Fi, or another Wi-Fi network.',
                'Check the port and security settings: incoming {imap_port} with SSL/TLS, outgoing {smtp_port} with {smtp_security}.',
                'Use the readiness check on this page to see whether the mail service is reachable.',
            ],
            'support' => 'If the readiness check shows a problem, your IT team needs to fix it; there is nothing to change on your phone.',
        ],
        'certificate-warning' => [
            'title' => 'The app shows a certificate or security warning',
            'cause' => 'The server name in the app does not match the server\'s certificate, or the network is intercepting the connection.',
            'steps' => [
                'Do not tap Continue, Trust or Accept. Cancel instead.',
                'Check that the server names in the app exactly match {imap_host} and {smtp_host}.',
                'Try again on mobile data. If the warning disappears, the Wi-Fi network may be unsafe.',
            ],
            'support' => 'If the names match and the warning remains, report it to your IT team immediately. Never turn off encryption or accept an untrusted certificate.',
        ],
        'cannot-send' => [
            'title' => 'I can receive mail but cannot send',
            'cause' => 'The outgoing server is missing sign-in, uses the wrong port, or the network blocks the port.',
            'steps' => [
                'In the outgoing server settings, turn on "Require sign-in" / authentication and use your full email address and password.',
                'Check the outgoing server is {smtp_host}, port {smtp_port}, security {smtp_security}.',
                'If it still fails, try the alternative: {smtp_alt}.',
                'Make sure you send from your work account, not your personal one.',
            ],
            'support' => 'If sending fails with both ports, tell your IT team the error message shown by the app.',
        ],
        'cannot-receive' => [
            'title' => 'New mail does not arrive on my phone',
            'cause' => 'The app checks for mail only every so often, background data is restricted, or the mailbox is full.',
            'steps' => [
                'Pull down on the inbox to refresh.',
                'In the app\'s account settings, set "Sync frequency" or "Fetch new data" to every 15 minutes or Push.',
                'Check whether the message arrived in webmail at {webmail_url}. If it is there, the app needs to sync.',
                'Allow the app to use background data and send notifications in your phone settings.',
            ],
            'support' => 'If the message is not in webmail either, ask the sender to check the address, or contact your IT team.',
        ],
        'mailbox-full' => [
            'title' => 'My mailbox is full',
            'cause' => 'Your mailbox has reached its storage limit, so new mail cannot be delivered.',
            'steps' => [
                'Delete large old messages and attachments, then empty the Trash folder.',
                'Download attachments you need to keep, then delete the messages.',
            ],
            'support' => 'Ask your IT team whether more storage is available for your mailbox.',
        ],
        'account-suspended' => [
            'title' => 'The account is disabled or suspended',
            'cause' => 'Your mailbox or your institution\'s account was disabled by an administrator.',
            'steps' => [
                'Sign in to webmail at {webmail_url} to see the exact message.',
            ],
            'support' => 'Only your institution can re-enable the mailbox. Contact your IT team.',
        ],
        'no-discovery' => [
            'title' => 'The app cannot find the settings automatically',
            'cause' => 'Many apps only know the settings of large providers. This is normal for institutional email.',
            'steps' => [
                'Choose manual setup and IMAP.',
                'Enter the settings shown on this page using the copy buttons.',
            ],
            'support' => 'Your IT team can publish optional automatic-setup DNS records, but manual setup always works.',
        ],
    ],
];
