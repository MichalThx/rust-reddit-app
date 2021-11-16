## **rust-reddit-app** ##


## What does it do and why?

The initial thought behind the project was to create a simple program for gathering data from reddit. The initial problem was with storing historical data, not only how popular post is but also how quickly it took him to get  there (same with comments). The usual use case would be feeding this data to data mining algorithm and finding sentiment / popularity of certain terms. 

## How to run the project?

There are 4 requirements for it to run:
1. Rust and cargo
2. Running postgre instance with 3 tables (based on the postgresql_tables.sql):
   1. Updates - for storing and setting update times for posts 
   2. Posts - storing data about posts 
   3. Comments - storing data about comments
3. Two configuration files
   1. configuration.toml which should be filled with data based on the information inside (and renamed configuration.toml from configuration_new.toml)
   2. tokens.json for storing Reddit tokens (and renamed tokens.json from tokens_new.json)
      1. Why does it not store in confugration.toml? The toml crate has a very limited support for date time objects and there is limited interoperability with chrono (other crate)
4. Reddit credentials
   1. Go to https://www.reddit.com/prefs/apps/ and create new app (you have to be logged in)

Once those requirements are cleared. You can run the project with 

`cargo run` from the folder (there will be DEBUG information displayed but it could be changed with DEBUG = false in the code)

## Further Reading

There will soon be a post about the whole process on https://michalszwedo.com (will edit the readme then)