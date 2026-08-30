# OxideForms

A small, self-hosted, **file-driven** form service, built with
[Rust](https://rust-lang.org/), [axum](https://github.com/tokio-rs/axum) (HTTP),
[askama](https://github.com/django-askama/askama) (templates),
[SQLite](https://sqlite.org/), and Tailwind CSS for a dark, modern UI.

Forms are **not** managed through an admin UI — they are plain `.json` files on
disk. Each file is one form; its **filename is the form's UUID** and therefore
its route.

## Quick start
As of now we only support deploying OxideForms with Docker.

### Docker Compose

The only thing you need from this repository is the Compose file — fetch it
with `wget` (or clone the repo):

```sh
wget https://raw.githubusercontent.com/fredericodeveloper/oxideforms/main/docker-compose.yml
```

Create a `.env` next to it (see the example below) and start:

```sh
mkdir -p forms data
docker compose up -d
```

A minimal `.env` looks like this (the repo ships a `.env.example` you can
copy): `cp .env.example .env`:

```ini
# .env
PORT=3000
ADMIN_PASSWORD=s3cret
```

Compose reads exactly two variables from `.env`:

* `PORT` — published on the host, mapped to `3000` inside the container.
  Defaults to `3000` if unset.
* `ADMIN_PASSWORD` — enables the `?admin=true` admin view; empty/omitted
  disables it.

On first start Compose pulls the image from the GitHub Container Registry —
no local build needed.

The service is addressed by a form's UUID. Hitting `/` on its own redirects to
the GitHub repository.

### Docker command

```sh
docker pull ghcr.io/fredericodeveloper/oxideforms:latest
docker run -d --name oxideforms -p 3000:3000 \
  -v "$PWD/forms:/forms" -v "$PWD/data:/data" \
  -e ADMIN_PASSWORD='s3cret' \
  ghcr.io/fredericodeveloper/oxideforms:latest
```

In both cases:

* `./forms` → `/forms` — the form `.json` files; add/edit them on the host and
  the container **hot-reloads** them within ~2 seconds.
* `./data` → `/data` — the SQLite database (submissions), persisted on disk.
* Port `3000` is published as `${PORT:-3000}`; the image pre-sets
  `HOST=0.0.0.0`, `FORMS_DIR=/forms`, `DB_PATH=/data/forms.db`.
* Set `ADMIN_PASSWORD` (in `.env` for Compose, or with `-e`) to enable the
  `?admin=true` admin view — unset, it is disabled.
* User-defined form content is **not** baked into the image
  (`.dockerignore` excludes `forms/`, `.env`, `*.db`); environment variables
  set at run time take precedence over any mounted `.env`.
* If your host runs SELinux enforcing, the container may be denied access to
  the bind-mounted directories (hot-reload and the database will fail); add
  a SELinux relabel option to both binds — `./forms:/forms:z` and
  `./data:/data:Z` in Compose, or `-v "$PWD/forms:/forms:z" -v
  "$PWD/data:/data:Z"` with `docker run` (`:z` shares the label with other
  containers; `:Z` keeps the database private)

## Defining a form

A form file looks like this:

```json
{
  "uuid": "8f14e45f-ceea-467f-8d3d-1890c9784b79",
  "title": "Contact Us",
  "description": "Optional intro text shown above the fields.",
  "fields": [
    { "id": "name",  "type": "text",     "label": "Full name", "required": true, "placeholder": "Ada Lovelace" },
    { "id": "email", "type": "email",    "label": "Email",     "required": true, "placeholder": "ada@example.com" },
    { "id": "topic", "type": "select",   "label": "Topic",     "required": true, "options": ["Support", "Feedback", "Other"] },
    { "id": "tags",  "type": "checkbox", "label": "Tags",      "options": ["Urgent", "Billing"] },
    { "id": "msg",   "type": "textarea", "label": "Message" }
  ]
}
```

* **`uuid`** — optional. If present it overrides the filename for routing.
  Omit it and the filename becomes the UUID.
* **`title`** / **`description`** — both optional (title defaults to the filename).
* **`single_submission`** — optional, defaults to `false`. When `true`, a
  visitor may answer the form only once: repeat submissions are rejected with
  an error, and revisiting the form shows the “response received” page instead.
  Visitors are tracked with a long-lived, random `HttpOnly` cookie (there is no
  user login to key on), so the limit is per browser — clearing the site's
  cookies lets the same person submit again.
* **`fields[].type`** — one of:
  `text`, `email`, `number`, `url`, `date`, `textarea`, `select`, `radio`, `checkbox`.
  Unknown values are rendered as plain text.
* **`fields[].options`** — required for `select` / `radio` / `checkbox`.
* **`fields[].required`** — makes the field mandatory (validation error if empty).
* **`pages`** — optional multi-page layout. Replace
  the flat `fields` list with an array of pages; each page has an optional
  `title` / `description` and its own `fields`. Visitors move through the pages
  one at a time with a progress bar and Back / Next / Submit buttons — answers
  are preserved while moving forward and back. Validation runs per page on
  “Next”, and in full on “Submit”. Omit it (or use plain `fields`) for a
  single-page form:

  ```json
  {
    "title": "Customer Survey",
    "pages": [
      { "title": "About you",   "fields": [ { "id": "name", "type": "text", "label": "Name", "required": true } ] },
      { "title": "Feedback",    "fields": [ { "id": "nps",  "type": "number", "label": "Rate us (1–10)", "required": true } ] }
    ]
  }
  ```
* **Creating a new form** — just write a new `<uuid>.json` file (generate a
  UUIDv4 for the name) into the forms directory; there is no builder UI, CLI
  or API. The running server picks it up automatically.

### Live reload

The forms directory is watched; add, edit, or delete a `.json` file and the
running server picks it up within ~2 seconds — no restart needed.

## Localization

The UI is currently available in English (default), Portuguese (Brazil), German, French,
Chinese, and Spanish.

## How the admin view works

1. Hit `<form url>?admin=true`.
2. If you're not authenticated, you see a password prompt.
3. Correct password → an `HttpOnly` session cookie is set and the responses
   table is shown. The token is an **HMAC-SHA256 signature** (key derived from
   the password), so it survives restarts but can't be forged without the
   password, and it expires after 8 hours.

Responses are stored in SQLite as a JSON object keyed by field id, so the
submissions table always reflects the form's current field labels and order.

## Notes & production considerations

* This is a single-process server intended for a trusted host (your own VPS,
  laptop, etc.). Put it behind TLS (e.g. Caddy/nginx) and set a strong
  `ADMIN_PASSWORD` before exposing it anywhere.

---

Made by [Frederico Queiroz](https://github.com/fredericodeveloper/oxideforms).
