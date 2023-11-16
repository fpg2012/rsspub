use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::error::Error;
use std::str::FromStr;
use std::sync::Arc;

use tokio::sync::Mutex;

#[derive(Debug, serde::Deserialize)]
struct Site {
    name: String,
    url: String,
}

#[derive(Debug, serde::Deserialize)]
struct DailyEpubConfig {
    sites: Vec<Site>,
    cache_file: PathBuf,
}


struct DailyEpub {
    config: DailyEpubConfig,
    visited_guids: Arc<Mutex<HashMap<String, HashSet<String>>>>,
    date: chrono::NaiveDate,
}

impl DailyEpubConfig {

    fn from_config_file(config_file: PathBuf) -> Self {
        let content: String = fs::read_to_string(config_file).unwrap();
        let daily_epub: DailyEpubConfig = toml::from_str(content.as_str()).unwrap();

        daily_epub
    }

}

impl DailyEpub {
    fn new(config: DailyEpubConfig) -> Self {
        let cache_file = config.cache_file.clone();
        let cache_content = fs::read_to_string(cache_file).unwrap();
        let visited_guids: HashMap<String, HashSet<String>> = serde_json::from_str(cache_content.as_str()).unwrap();
        let date = chrono::Local::now().date_naive();

        DailyEpub {
            config,
            visited_guids: Arc::new(Mutex::new(visited_guids)),
            date,
        }
    }

    async fn get_rss(url: String) -> Result<rss::Channel, Box<dyn Error>> {
        let content = reqwest::get(url)
        .await?.bytes().await?;
        
        let channel = rss::Channel::read_from(&content[..])?;
        
        Ok(channel)
    }

    fn prepare_dir(&self, site: &Site) -> PathBuf {
        let mut path = PathBuf::from_str(".").unwrap();
        path.push(self.date.to_string());
        // path.push(site.name.clone());
        fs::create_dir_all(&path).unwrap();

        path
    }

    async fn generate_site(&self, site: &Site) -> Result<(), Box<dyn Error>> {
        let path = self.prepare_dir(site);
        let rss = Self::get_rss(site.url.clone()).await?;
        println!("{}: {}", &site.name, &rss.title);

        let mut new_guids: HashSet<String> = HashSet::new();

        let vis = self.visited_guids.clone();

        let n = rss.items.len();
        let mut i = 1;
        
        for item in rss.items {
            println!("{}/{}", &i, &n);
            if let (Some(guid), Some(title), Some(description)) = (item.guid, item.title, item.description) {
                // ensure the page is new
                if let Some(site_visited_guids) = vis.lock().await.get(&site.name) {
                    if site_visited_guids.get(guid.value().clone()) != None {
                        continue;
                    }
                }
                println!("[gen] {}, {}", title, guid.value);
                let mut item_path = path.clone();
                item_path.push(format!("{}-{}.html", site.name, title));
    
                let contents = format!{"
                <!DOCTYPE html>
                <head>
                    <meta charset=\"utf-8\">
                    <title>{}-{}</title>
                </head>
                <body>
                <h1>{}-{}</h1>
                {}
                </body>
                ", site.name, title, site.name, title, description};
    
                fs::write(item_path, contents)?;

                new_guids.insert(String::from(guid.value().clone()));

                println!("[done] {}, {}", title, guid.value);
            }
            i += 1;
        }

        if vis.lock().await.get(&site.name) != None {
            vis.lock().await.get_mut(&site.name).unwrap().extend(new_guids);
        } else {
            vis.lock().await.insert(site.name.clone(), new_guids);
        }

        Ok(())
    }

    async fn write_back_visited_guids(&self) {
        let vis = serde_json::to_string_pretty(&self.visited_guids.clone().lock().await.clone()).unwrap();
        fs::write(&self.config.cache_file, vis).unwrap();
    }

    async fn generate(&self) -> Result<(), Box<dyn Error>> {
        for site in self.config.sites.iter() {
            self.generate_site(site).await?;
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    println!("Hello, world!");
    let config = DailyEpubConfig::from_config_file(PathBuf::from_str("./config.toml").unwrap());
    let dayepub = DailyEpub::new(config);
    dayepub.generate().await.unwrap();
    println!("all done, write back visited guids");
    dayepub.write_back_visited_guids().await;
}
