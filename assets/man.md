# nonbin(1)

## NAME

nonbin - a minimal paste service

## USAGE

### `GET /<id>/[/<file name>]`

get a paste's contents by ID and optional file name.

### `POST /`

upload a paste.

if the request's Content-Type is multipart/form-data, then the first multipart field in the request will be uploaded as a file.

if successful, the server will respond with 201 Created and a JSON body like the following:

```json
{
    "id": "<id>",
    "url": "<file url>",
    "delete_key": "<delete key>"
}
```

the secret delete key can be used to delete the paste at a later date.

### `DELETE /<id>?delete_key=<delete_key>`

delete a paste by ID and delete key.

## NOTES

pastes expire in 72 hours.

EXIF data is not stripped.

if you run into any problems, send an email to to help@[this domain name] or open a ticket in the issue tracker.

## SOURCE

<https://git.sr.ht/~cassaundra/nonbin> (AGPL v3)
