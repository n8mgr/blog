# ~/n8m.us

## Run

```sh
cargo run
```

Open <http://localhost:3000>.

## Add a post

Create `site/posts/YYYY-MM-DD-slug.md`:

```markdown
---
{
  "title": "Post title",
  "description": "A short summary",
  "tags": ["rust", "web"]
}
---

# Post title

Write Markdown here.
```

The filename date is used as the publication date at midnight UTC. Rebuild after changing content.
