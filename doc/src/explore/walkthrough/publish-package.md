---
title: Publish a Package
---

Use the `sui_publish` method to publish packages, providing the necessary parameter values:
* `sender` - The transaction signer's Sui address.
* `compiled_modules` - The compiled bytes of a Move module. In this case, the response received in the previous call.
* `gas` - Gas object to use.
* `gas_budget`. The gas budget.

As has become custom, bind the base64 response to a variable, making sure not to include the enclosing square brackets:

```sh
# Bind the base64 output without the square brackets []
module_base64="oRzrCwUAAAAKAQAIAggQAxgpBEEEBUUsB3F9CO4BKAqWAhIMqAJyDZoDBgAAAQEBAgEDAAQIAAAFDAADBgIAAQ0EAAAHAAEAAAgCAwAACQIDAAAKBAEAAAsFAwABDgAHAAMPCAkAAgIKAQEIBwYHCwEHCAIAAQYIAQEDBQcIAAMDBQcIAgEGCAABCAABCAMBBggCAQUCCQAFAQgBCW15X21vZHVsZQZvYmplY3QIdHJhbnNmZXIKdHhfY29udGV4dAVGb3JnZQVTd29yZAlUeENvbnRleHQEaW5pdAVtYWdpYwhzdHJlbmd0aAxzd29yZF9jcmVhdGUOc3dvcmRzX2NyZWF0ZWQCaWQDVUlEA25ldwZzZW5kZXIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACAAICDAgDCwMBAgMMCAMIAwkDAAAAAAYLCgARBQYAAAAAAAAAABIADAELAQsALhEGOAACAQEAAAEECwAQABQCAgEAAAEECwAQARQCAwEEAAsSCwQRBQsBCwISAQwFCwULAzgBCgAQAhQGAQAAAAAAAAAWCwAPAhUCBAEAAAEECwAQAhQCAQEBAgABAA=="

# Prepare the data
data="{\"jsonrpc\": \"2.0\", \"id\": 1, \"method\": \"sui_publish\", \"params\": [\"$address\", [\"$module_base64\"], \"$gas_id\", 10000]}"

# Fire the request
curl -X POST -H 'Content-type: application/json' --data-raw "$data" $rpc > result.json
```

If successful, the response includes a `txBytes` value:

```JSON
{
    "jsonrpc": "2.0",
    "result": {
        "txBytes": "VHJhbnNhY3Rpb25EYXRhOjoAAQHMA6Ec6wsFAAAACgEACAIIEAMYKQRBBAVFLAdxfQjuASgKlgISDKgCcg2aAwYAAAEBAQIBAwAECAAABQwAAwYCAAENBAAABwABAAAIAgMAAAkCAwAACgQBAAALBQMAAQ4ABwADDwgJAAICCgEBCAcGBwsBBwgCAAEGCAEBAwUHCAADAwUHCAIBBggAAQgAAQgDAQYIAgEFAgkABQEIAQlteV9tb2R1bGUGb2JqZWN0CHRyYW5zZmVyCnR4X2NvbnRleHQFRm9yZ2UFU3dvcmQJVHhDb250ZXh0BGluaXQFbWFnaWMIc3RyZW5ndGgMc3dvcmRfY3JlYXRlDnN3b3Jkc19jcmVhdGVkAmlkA1VJRANuZXcGc2VuZGVyAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgACAgwIAwsDAQIDDAgDCAMJAwAAAAAGCwoAEQUGAAAAAAAAAAASAAwBCwELAC4RBjgAAgEBAAABBAsAEAAUAgIBAAABBAsAEAEUAgMBBAALEgsEEQULAQsCEgEMBQsFCwM4AQoAEAIUBgEAAAAAAAAAFgsADwIVAgQBAAABBAsAEAIUAgEBAQIAAQD8CL+O/MPbNiGKmjFf9segvw09EunIWn9JwNBlJoVpDb1VL+F6g70uAwAAAAAAAAAgzMc4fVHWOg4vVMa8l1JM4erSJKxz7I8BlluS5dWKoZEBAAAAAAAAABAnAAAAAAAA"
        //...
    } 
    //...
}
```

Now to execute the transaction:

```sh
# Usual binding
tx_bytes="VHJhbnNhY3Rpb25EYXRhOjoAAQHMA6Ec6wsFAAAACgEACAIIEAMYKQRBBAVFLAdxfQjuASgKlgISDKgCcg2aAwYAAAEBAQIBAwAECAAABQwAAwYCAAENBAAABwABAAAIAgMAAAkCAwAACgQBAAALBQMAAQ4ABwADDwgJAAICCgEBCAcGBwsBBwgCAAEGCAEBAwUHCAADAwUHCAIBBggAAQgAAQgDAQYIAgEFAgkABQEIAQlteV9tb2R1bGUGb2JqZWN0CHRyYW5zZmVyCnR4X2NvbnRleHQFRm9yZ2UFU3dvcmQJVHhDb250ZXh0BGluaXQFbWFnaWMIc3RyZW5ndGgMc3dvcmRfY3JlYXRlDnN3b3Jkc19jcmVhdGVkAmlkA1VJRANuZXcGc2VuZGVyAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgACAgwIAwsDAQIDDAgDCAMJAwAAAAAGCwoAEQUGAAAAAAAAAAASAAwBCwELAC4RBjgAAgEBAAABBAsAEAAUAgIBAAABBAsAEAEUAgMBBAALEgsEEQULAQsCEgEMBQsFCwM4AQoAEAIUBgEAAAAAAAAAFgsADwIVAgQBAAABBAsAEAIUAgEBAQIAAQD8CL+O/MPbNiGKmjFf9segvw09EunIWn9JwNBlJoVpDb1VL+F6g70uAwAAAAAAAAAgzMc4fVHWOg4vVMa8l1JM4erSJKxz7I8BlluS5dWKoZEBAAAAAAAAABAnAAAAAAAA"

# Get the signature
sui keytool sign --address "$address" --data "$tx_bytes"
# INFO sui::keytool: Address : 0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12
# INFO sui::keytool: Flag Base64: AA==
# INFO sui::keytool: Public Key Base64: R904IKMQHbULGI+8g3aKNndZHcXbO3FSRoZF3QspcnY=
# INFO sui::keytool: Signature : 73nBz+KZ3ppddt/gc4KBWePE6maYlwgpIgqozSkd4V6HkyFJt2NRy/oD82to8HnlDzzDECNgATSM2YyNDx9fBw==

# More biding
signature="73nBz+KZ3ppddt/gc4KBWePE6maYlwgpIgqozSkd4V6HkyFJt2NRy/oD82to8HnlDzzDECNgATSM2YyNDx9fBw=="

# Prepare data
data="{\"jsonrpc\": \"2.0\", \"id\": 1, \"method\": \"sui_executeTransaction\", \"params\": [\"$tx_bytes\", \"$scheme\",\"$signature\",\"$pub_key\",\"WaitForLocalExecution\"]}"

# Fire
curl -X POST -H 'Content-type: application/json' --data-raw "$data" $rpc > result.json
```

If successful, you get a similar response:

```JSON
{
    // ...
    "created": [
                {
                    "owner": {
                        "AddressOwner": "0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12"
                    },
                    "reference": {
                        "objectId": "0xeb8e4a532d09c596564f1c48b17f8ca4b339dbda", // Our new Forge object
                        "version": 1,
                        "digest": "Bp1deLh2HeD/m0CdW659evPsE8Hr/ynxQMGfFq9+Aaw="
                    }
                },
                {
                    "owner": "Immutable",
                    "reference": {
                        "objectId": "0x9238689c6c14db84519686958643bf47de13e54e", // Our new package
                        "version": 1,
                        "digest": "xXms6tXr52jaiHfCeVeUmgjTqPGWGrUan8cFzHjw3NA="
                    }
                }
    ]
    // ...
}
```

A new object and a new package appear on the Sui ledger. As the comments in the JSON point out, there is both a Forge object and a package. The package contains the functions to create swords and check them, while the package's `init` function created the Forge object. The `init` function runs only once, right after the bytecode is stored in Sui.

With a Forge object now available, it's time to mint some swords to raise some mayhem!