# Berry Pasty

A simple **pure Rust** pastebin and url shortener, inspired by [pasty](https://github.com/darkyzhou/pasty).

> [!TIP]
>
> **The image size is only 3.9MiB (1.77MiB compressed), currently support x86_64, aarch64.**
>
> Theoretically, it can be compiled to any Rust-supported architecture and run as a single binary.

## Usage

### Access

- **Index**

    You can set the index text or link in the configuration file, if the index link is specified, it will redirect to the link directly.

- **Access a paste**

    To access a paste, you can just visit the link.

    ```bash
    curl http://localhost:8080/<id>
    ```

### Update

To update (or add) a paste, you can send a POST request to the server with the content in the body, the server will return the link to the paste.

- **Update a text paste**

    To update (or add) a text paste, you can send a POST request to the server with the content in the body, the server will return the link to the paste.

    ```bash
    curl --data "Hello, World!" http://localhost:8080/access=<access_password>
    ```

- **Update a redirect link**

    To update (or add) a redirect link, you should specify the type as `link` and the link in the body.

    ```bash
    curl --data "https://example.com" http://localhost:8080/access=<access_password>&type=link
    ```

- **Update a paste with a specific ID**

    To update (or add) a paste with a specific ID (or path), you can just specify the ID in the URL.

    ```bash
    curl --data "Hello, World!" http://localhost:8080/<id>?access=<access_password>
    ```

- **Update a paste with a password**

    To update (or add) a paste with a password, you can specify the password in the URL.

    **Only the owner can edit or delete the paste with the password.**

    ```bash
    curl --data "Hello, World!" http://localhost:8080/<id>?access=<access_password>&password=<password>
    ```

### Delete

To delete a paste, you can send a DELETE request to the server with the password in the URL.

```bash
curl -X DELETE http://localhost:8080/<id>?access=<access_password>&password=<password>
```

### Statistics and Management

All data is returned in JSON format. you can use `curl` and pipe it to `python -m json.tools` to get a better view.

- **Statistics**

    You can visit the `<id>/stats` path to get the statistics of the paste.

- **List**

    You can visit the `/all?access=<access_password>` path to get a list of all pastes and its statistics.

## Configuration

```toml
[default]
address = "0.0.0.0"

[default.limits]
# only allow 128 KiB of data to be uploaded
"plain/text" = "128 KiB"

[default.pasty]
# the path to the database file
db_path = "berry-pasty.redb"

# the password required to add or delete, set to "" if not needed
access_password = "password"

# the length of the random ID for pastes
random_id_length = 8

# The text displayed on the index page,
# if index_link is specified, it will redirect to the link directly
index_text = "Welcome Berry Pasty！"
index_link = ""
```
