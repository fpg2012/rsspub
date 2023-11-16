use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::error::Error;
use std::str::FromStr;
use std::sync::Arc;
use subprocess::Exec;

use tokio::sync::Mutex;

#[derive(Debug, serde::Deserialize, Clone)]
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

    fn prepare_dir(&self) -> PathBuf {
        let mut path = PathBuf::from_str(".").unwrap();
        path.push(self.date.to_string());
        // path.push(site.name.clone());
        fs::create_dir_all(&path).unwrap();
        path
    }

    async fn generate_site(site: Site, path: PathBuf, vis: Arc<Mutex<HashMap<String, HashSet<String>>>>) -> Result<(), Box<dyn Error>> {
        
        let rss = Self::get_rss(site.url.clone()).await?;
        println!("{}: {}", &site.name, &rss.title);

        let mut new_guids: HashSet<String> = HashSet::new();

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
                    <title>[{}]{}</title>
                </head>
                <body>
                <h1>[{}]{}</h1>
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

    async fn generate(&self) -> Result<PathBuf, Box<dyn Error>> {
        let path = self.prepare_dir();
        let vis = self.visited_guids.clone();

        let mut join_set = tokio::task::JoinSet::new();
        for site in self.config.sites.iter() {
            let path = path.clone();
            let site = site.clone();
            let vis = vis.clone();
            join_set.spawn(async move {
                Self::generate_site(site, path, vis).await.unwrap();          
            });
        }

        // join
        while let Some(result) = join_set.join_next().await {
            continue;
        }
        
        Ok(path)
    }
}

#[tokio::main]
async fn main() {
    println!("Hello, world!");
    let config = DailyEpubConfig::from_config_file(PathBuf::from_str("./config.toml").unwrap());
    let dayepub = DailyEpub::new(config);
    let path = dayepub.generate().await.unwrap();
    println!("all done, write back visited guids");
    let path_str = path.to_string_lossy();
    let date = chrono::Local::now().date_naive();
    let command = format!("pandoc -s -i {}/*.html -o {}.epub --toc --title \"DailyNews-{}\" --epub-cover-image ./cover.png", path_str, path_str, date.to_string());
    println!("{}", command);
    let pandoc_handle = Exec::cmd(command);
    dayepub.write_back_visited_guids().await;
    pandoc_handle.join().unwrap();
}
