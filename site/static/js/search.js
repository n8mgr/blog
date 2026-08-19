(() => {
  const input = document.querySelector("#search");
  const list = document.querySelector("#posts-list");
  const status = document.querySelector("#search-status");
  if (!input || !list || !status) return;

  const rows = new Map(
    [...list.querySelectorAll("[data-slug]")].map((row) => [row.dataset.slug, row]),
  );

  fetch("/search-index.json")
    .then((response) => {
      if (!response.ok) throw new Error(`search index returned ${response.status}`);
      return response.json();
    })
    .then((posts) => {
      const searchable = posts.map((post) => ({
        ...post,
        text: `${post.title} ${post.description} ${post.tags.join(" ")}`.toLocaleLowerCase(),
      }));

      const filter = () => {
        const query = input.value.trim().toLocaleLowerCase();
        let matches = 0;
        for (const post of searchable) {
          const row = rows.get(post.slug);
          if (!row) continue;
          const visible = !query || post.text.includes(query);
          row.hidden = !visible;
          if (visible) matches += 1;
        }
        status.textContent = query
          ? `${matches} ${matches === 1 ? "post" : "posts"} found`
          : "";
      };

      input.addEventListener("input", filter);
    })
    .catch((error) => {
      console.error(error);
      status.textContent = "Search is temporarily unavailable; all posts are shown.";
    });
})();
