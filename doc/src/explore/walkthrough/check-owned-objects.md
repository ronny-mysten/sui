---
title: Check the objects our address owns
---

*In Sui everything is an object.* Sui is object-centric and every object owns a UID. You might not be aware yet, but if you followed the previous steps then your address owns 5 objects that contain a specific amount of SUI tokens tapped from the Discord faucet. When considering SUI tokens, you can think of each Coin object as SUI banknotes. As such, your address actually has 5 banknotes, each with its own, currently equal denomination. 

If you want to transfer a SUI amount that is less than the denominated value of your SUI banknote, then you have to make change. To do this, you must split your large banknote into smaller denominations. For example, if you want to send 20,000 SUI to someone but all your banknotes contain 10,000,000 SUI, then you must split one of those banknotes into two smaller SUI banknotes, one with 20,000 SUI and another containing 9,980,000 SUI. You can then complete the transfer with your newly minted 20,000 SUI. In the end, you created two new SUI banknotes and deleted the original one. The actual process of splitting SUI objects typically completes without any direct involvement on your part, but is useful to understand.

As mentioned, your address should now have SUI assigned. To check, you can make an RPC call using the [`sui_getObjectsOwnedByAddress`](/sui-jsonrpc#sui_getObjectsOwnedByAddress). For ease of use on a command line, save the JSON payload to a variable before making the actual call.

```sh
data="{\"jsonrpc\": \"2.0\", \"method\": \"sui_getObjectsOwnedByAddress\", \"id\": 1, \"params\": [\"$address\"]}"
```
Make the call using cURL:

```sh
curl -X POST -H 'Content-type: application/json' --data-raw "$data" $rpc
```

If successful, the JSON response has the following shape:

```JSON
{
    "jsonrpc": "2.0",
    "result": [
        {
            "objectId": "0x91be7b2c011125f748c308872d00c8f200cabe15",
            "version": 1,
            "digest": "Zusbw800Wk4+vpkei8JQD+EJiQEf8PvmdIQokHwX0N0=",
            "type": "0x2::coin::Coin<0x2::sui::SUI>",
            "owner": {
                "AddressOwner": "0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12"
            },
            "previousTransaction": "B2015vW4kYmFXLsBw/njGWUOQHdmszg8jsdygrUFTPw="
        },
        {
            "objectId": "0x9f0afe6a6c3d72d281003b6f87ea84703113744c",
            "version": 1,
            "digest": "m0M5XSe4MLQWsz1mAPFAH8iztx8VHnZPNqYtCb4fc1k=",
            "type": "0x2::coin::Coin<0x2::sui::SUI>",
            "owner": {
                "AddressOwner": "0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12"
            },
            "previousTransaction": "3DEKooZaVHWiMrdjMb8e1h2kx2J+M9Yu5uJy2CHmgJ4="
        },
        {
            "objectId": "0xb2a2f77bbddb5c9c84a2abbfdd282d609274f96b",
            "version": 1,
            "digest": "w3U7ZJsnpcLB+NPiFTGrOz58I/a9TRz0YPGFRPdjm6o=",
            "type": "0x2::coin::Coin<0x2::sui::SUI>",
            "owner": {
                "AddressOwner": "0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12"
            },
            "previousTransaction": "E5CGmStqkRq7fB6MPwLx5MBbR8MNtzFEiNVRnfNL3s8="
        },
        {
            "objectId": "0xbb2d9b82ecc54012640d3cef122de4f394f8aa15",
            "version": 1,
            "digest": "dvRriMle+XIC9WHN0nEj+KD5rPWhLYBeXeFLAeCDSu8=",
            "type": "0x2::coin::Coin<0x2::sui::SUI>",
            "owner": {
                "AddressOwner": "0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12"
            },
            "previousTransaction": "bKioOVWJBniAlT1SlVUsspIiDDZABS+nvROSu3/RZmA="
        },
        {
            "objectId": "0xe9c85a7f49c0d0652685690dbd552fe17a83bd2e",
            "version": 1,
            "digest": "BptpMcUxbtjN0D9CGvtqSwS9odmEfW9pho+aIF/MiA4=",
            "type": "0x2::coin::Coin<0x2::sui::SUI>",
            "owner": {
                "AddressOwner": "0xfc08bf8efcc3db36218a9a315ff6c7a0bf0d3d12"
            },
            "previousTransaction": "lBnZ6OXExGfd0C5EzCLlR2gXAT/aTn0EnOvwF5KJEOU="
        }
    ],
    "id": 1
}
```

> **NOTE:** The response you receive in the terminal is not prettified like this example output, so it's a little difficult to parse. From this point forward, the example commands save the output to a JSON file so that you can more readily format the response if you're following along.

The details of the response offers some observations into objects on the Sui network. For instance, 
* `objectId` - Each object in the result has a UID assigned to its `objectId` parameter. This ID is unique for each object in the Sui network. 
* `type` - Each object also has a `type` parameter set to `0x2::coin::Coin<0x2::sui::SUI`, where the `0x2` address can contain a number of packages that are native to Sui, like the SUI Coin module, the transfer function, the `TxContext` global object, and so on. In the case of this example, each object has a `type` of `0x2::sui::SUI`, which are SUI tokens. 
* `owner` - The `owner` object provides the address of the object's current owner.
* `previousTransaction` - Every object has a `previousTransaction` value to verify its placement on-chain.
* `version` - The `version` value reports the number of transactions an object has been through. Because all the coins in this example are freshly minted by faucet, they have experienced a single transaction so far: `"version": 1`.

Your address now has Coins assigned to it and you have retrieved a list of those objects. In the next example, you take a closer look at one of those Coins.  