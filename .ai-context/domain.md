# Domain Knowledge

- **Telemetry Categories:**
  1. `system`: CPU, RAM, active window titles, USB events.
  2. `network`: Visited FQDNs (NO full URLs).
  3. `productivity`: Keystrokes/clicks frequency (NO keylogging), and screenshots.
  4. `files`: File metadata changes (creation/modification).
  5. `location`: GPS coordinates (Only for FIELD workers during work hours).

- **Worker Roles:**
  - `OFFICE`: Cannot be tracked via GPS.
  - `FIELD`: Can be tracked via GPS within `work_hours`.
