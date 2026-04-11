ALTER TABLE host_stats ADD COLUMN init_system TEXT CHECK (init_system IN ('systemd', 'openrc', 'self-extracting-shell', 'self-extracting-pwsh', 'launchd'));
ALTER TABLE host_stats ADD COLUMN os TEXT CHECK (os IN ('linux', 'macos', 'windows'));
