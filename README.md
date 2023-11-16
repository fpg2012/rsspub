# rsspub

convert RSS feeds to an epub book with pandoc for reading on kindle

## Example

> Ensure pandoc is installed and added to PATH.

1. create `test/config.toml`

```toml
sites = [
    { name = "Empty Space", url = "https://nth233.top/feed.xml" },
    { name = "Empty Space (notes)", url = "https://nth233.top/notes/rss.xml"},
]

cache_file = "cache.json"
```

3. create `test/cache.json`

```json
{}
```

4. `cd` into `test` and run

```
cargo run
```
