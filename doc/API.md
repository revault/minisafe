# Liana daemon API

`lianad` exposes a [JSON-RPC 2.0](https://www.jsonrpc.org/specification)
interface over a Unix Domain socket.

Commands must be sent as valid JSONRPC 2.0 requests, ending with a `\n`.

| Command                                                     | Description                                                   |
| ----------------------------------------------------------- | ----------------------------------------------------          |
| [`stop`](#stop)                                             | Stops liana daemon                                            |
| [`getinfo`](#getinfo)                                       | Get general information about the daemon                      |
| [`getnewaddress`](#getnewaddress)                           | Get a new receiving address                                   |
| [`listaddresses`](#listaddresses)                           | List addresses given start_index and count                     |
| [`listcoins`](#listcoins)                                   | List all wallet transaction outputs.                          |
| [`createspend`](#createspend)                               | Create a new Spend transaction                                |
| [`updatespend`](#updatespend)                               | Store a created Spend transaction                             |
| [`listspendtxs`](#listspendtxs)                             | List all stored Spend transactions                            |
| [`delspendtx`](#delspendtx)                                 | Delete a stored Spend transaction                             |
| [`broadcastspend`](#broadcastspend)                         | Finalize a stored Spend PSBT, and broadcast it                |
| [`rbfpsbt`](#rbfpsbt)                                       | Create a new RBF Spend transaction                            |
| [`startrescan`](#startrescan)                               | Start rescanning the block chain from a given date            |
| [`listconfirmed`](#listconfirmed)                           | List of confirmed transactions of incoming and outgoing funds |
| [`listtransactions`](#listtransactions)                     | List of transactions with the given txids                     |
| [`createrecovery`](#createrecovery)                         | Create a recovery transaction to sweep expired coins          |
| [`updatelabels`](#updatelabels)                             | Update the labels                                             |
| [`getlabels`](#getlabels)                                   | Get the labels for the given addresses, txids and outpoints   |

# Reference

## General

### `stop`

Stops the Liana daemon.

#### Response

Returns an empty response.

| Field         | Type   | Description |
| ------------- | ------ | ----------- |

### `getinfo`

General information about the daemon

#### Request

This command does not take any parameter for now.

| Field         | Type              | Description                                                 |
| ------------- | ----------------- | ----------------------------------------------------------- |

#### Response

| Field                | Type          | Description                                                                                  |
| -------------------- | ------------- | -------------------------------------------------------------------------------------------- |
| `version`            | string        | Version following the [SimVer](http://www.simver.org/) format                                |
| `network`            | string        | Answer can be `mainnet`, `testnet`, `regtest`                                                |
| `block_height`       | integer       | The block height we are synced at.                                                           |
| `sync`               | float         | The synchronization progress as percentage (`0 < sync < 1`)                                  |
| `descriptors`        | object        | Object with the name of the descriptor as key and the descriptor string as value             |
| `rescan_progress`    | float or null | Progress of an ongoing rescan as a percentage (between 0 and 1) if there is any              |
| `timestamp`          | integer       | Unix timestamp of wallet creation date                                                       |

### `getnewaddress`

Get a new address for receiving coins. This will always generate a new address regardless of whether
it was used or not.

#### Request

This command does not take any parameter for now.

| Field         | Type              | Description                                                 |
| ------------- | ----------------- | ----------------------------------------------------------- |

#### Response

| Field                  | Type    | Description                           |
| ---------------------- | ------- | ------------------------------------- |
| `address`              | string  | A Bitcoin address                     |
| `derivation_index`     | integer | The derivation index for this address |


### `listaddresses`

List receive and change addresses given start_index and count. Both arguments are optional.
Default value for `start_index` is 0.
If no value is passed for `count` the maximum generated index between receive and change is selected.

#### Request

| Field         | Type              | Description                                                 |
| ------------- | ----------------- | ----------------------------------------------------------- |
| `start_index` | integer(optional) | Index of the first address to list                          |
| `count`       | integer(optional) | Number of addresses to list                                 |

#### Response

| Field         | Type              | Description                                                 |
| ------------- | ----------------- | ----------------------------------------------------------- |
| `index`       | integer           | Derivation index                                            |
| `receive`     | string            | Receive address                                             |
| `change`      | string            | Change address                                              |


### `listcoins`

List all our transaction outputs, optionally filtered by status and/or outpoint.

#### Request

| Field          | Type              | Description                                                       |
| -------------- | ----------------- | ----------------------------------------------------------------- |
| `statuses`     | list of string    | List of statuses to filter coins by (see below).                  |
| `outpoints`    | list of string    | List of outpoints to filter coins by, as `txid:vout`.             |

A coin may have one of the following four statuses:
- `unconfirmed`: deposit transaction has not yet been included in a block and coin has not been included in a spend transaction
- `confirmed`: deposit transaction has been included in a block and coin has not been included in a spend transaction
- `spending`: coin (whose deposit transaction may not yet have been confirmed) has been included in an unconfirmed spend transaction
- `spent`: coin has been included in a confirmed spend transaction

#### Response

| Field              | Type          | Description                                                                                                        |
| ------------------ | ------------- | ------------------------------------------------------------------------------------------------------------------ |
| `address`          | string        | Address containing the script pubkey of the coin                                                                   |
| `amount`           | int           | Value of the TxO in satoshis.                                                                                      |
| `derivation_index` | int           | Derivation index used to create the coin deposit address.                                                          |
| `outpoint`         | string        | Transaction id and output index of this coin.                                                                      |
| `block_height`     | int or null   | Block height the transaction was confirmed at, or `null`.                                                          |
| `spend_info`       | object        | Information about the transaction spending this coin. See [Spending transaction info](#spending_transaction_info). |
| `is_immature`      | bool          | Whether this coin was created by a coinbase transaction that is still immature.                                    |
| `is_change`        | bool          | Whether the coin deposit address was derived from the change descriptor.                                           |


##### Spending transaction info

| Field      | Type        | Description                                                    |
| ---------- | ----------- | -------------------------------------------------------------- |
| `txid`     | str         | Spending transaction's id.                                     |
| `height`   | int or null | Block height the spending tx was included at, if confirmed.    |


### `createspend`

Create a transaction spending one or more of our coins. All coins must exist and not be spent.

If no coins are specified in `outpoints`, they will be selected automatically from the set of
confirmed coins (see [`listcoins`](#listcoins) for coin status definitions).

Will error if the given coins are not sufficient to cover the transaction cost at 90% (or more) of
the given feerate. If on the contrary the transaction is more than sufficiently funded, it will
create a change output when economically rationale to do so.

You can create a send-to-self transaction by not specifying any destination. This command will
create a single change output. This may be useful to "refresh" coins whose timelocked recovery path
may be close to expiry without having to bear the complexity of computing the correct amount for the
change output.

The optional `change_address` parameter allows the caller to specify what address to use for the
leftover funds after all destinations have been set. This can be used to "sweep" the wallet: use all
the unspent coins as input, set the other destination(s), if any, then set the `change_address` to
the address of the wallet to sweep the funds to. Note however this output would only be created if
there is enough remaining funds after sending to the specified destinations. This command WILL NOT
ERROR if there isn't enough leftover funds to create the change/sweep output.

This command will refuse to create any output worth less than 5k sats.

#### Request

| Field            | Type              | Description                                                       |
| ---------------- | ----------------- | ----------------------------------------------------------------- |
| `destinations`   | object            | Map from Bitcoin address to value.                                |
| `outpoints`      | list of string    | List of the coins to be spent, as `txid:vout`.                    |
| `feerate`        | integer           | Target feerate for the transaction, in satoshis per virtual byte. |
| `change_address` | string            | Address to be used for leftover amount, if any.                   |

#### Response

If the spend is created successfully, the following response will be received:

| Field          | Type              | Description                                          |
| -------------- | ----------------- | ---------------------------------------------------- |
| `psbt`         | string            | PSBT of the spending transaction, encoded as base64. |
| `warnings`     | list of string    | Warnings, if any, generated during spend creation.   |

If there are insufficient funds to create the required spend, then the following response will be received:

| Field          | Type              | Description                                          |
| -------------- | ----------------- | ---------------------------------------------------- |
| `missing`      | integer           | Additional sats required to create the spend.        |

### `updatespend`

Store the PSBT of a Spend transaction in database, updating it if it already exists.

Will merge the partial signatures for all inputs if a PSBT for a transaction with the same txid
exists in DB.

#### Request

| Field     | Type   | Description                                 |
| --------- | ------ | ------------------------------------------- |
| `psbt`    | string | Base64-encoded PSBT of a Spend transaction. |

#### Response

This command does not return anything for now.

| Field          | Type      | Description                                          |
| -------------- | --------- | ---------------------------------------------------- |


### `listspendtxs`

List stored Spend transactions.

#### Request

This command does not take any parameter for now.

| Field         | Type              | Description                                                 |
| ------------- | ----------------- | ----------------------------------------------------------- |

#### Response

| Field          | Type          | Description                                                      |
| -------------- | ------------- | ---------------------------------------------------------------- |
| `spend_txs`    | array         | Array of Spend tx entries                                        |

##### Spend tx entry

| Field          | Type              | Description                                                             |
| -------------- | ----------------- | ----------------------------------------------------------------------- |
| `psbt`         | string            | Base64-encoded PSBT of the Spend transaction.                           |
| `updated_at`   | int or null       | UNIX timestamp of the last time this PSBT was updated.                  |


### `delspendtx`

#### Request

| Field    | Type   | Description                                         |
| -------- | ------ | --------------------------------------------------- |
| `txid`   | string | Hex encoded txid of the Spend transaction to delete |

#### Response

This command does not return anything for now.

| Field          | Type      | Description                                          |
| -------------- | --------- | ---------------------------------------------------- |

### `broadcastspend`

#### Request

| Field    | Type   | Description                                            |
| -------- | ------ | ------------------------------------------------------ |
| `txid`   | string | Hex encoded txid of the Spend transaction to broadcast |

#### Response

This command does not return anything for now.

| Field          | Type      | Description                                          |
| -------------- | --------- | ---------------------------------------------------- |

### `rbfpsbt`

Create PSBT to replace the given transaction, which must point to a PSBT in our database, using RBF.

This command can be used to either:
- "cancel" the transaction: the replacement will include at least one input from the previous transaction and will have only
a single output (change).
- bump the fee: the replacement will include all inputs from the previous transaction and all non-change outputs
will be kept the same, with only the change amount being modified as required.

In both cases, the replacement transaction may include additional confirmed coins as inputs if required
in order to pay the higher fee (this applies also when replacing a self-send).

If the transaction includes a change output to one of our own change addresses,
this same address will be used for change in the replacement transaction, if required.

If the transaction pays to more than one of our change addresses, then the one receiving the highest value
will be used as a change address in the replacement and the others will be treated as non-change outputs
(i.e. removed for cancel or otherwise kept the same).

If `feerate` is not passed to the command, the target feerate of the replacement will be set to the minimum value
allowed in order to replace this transaction using RBF (see https://github.com/bitcoin/bitcoin/blob/master/doc/policy/mempool-replacements.md#current-replace-by-fee-policy for further details about this and other conditions that must be satisfied when using RBF).

#### Request

| Field       | Type              | Description                                                     |
| ----------- | ----------------- | --------------------------------------------------------------- |
| `txid`      | string            | Hex encoded txid of the Spend transaction to be replaced.       |
| `is_cancel` | bool              | Whether to "cancel" the transaction or simply bump the fee.     |
| `feerate`   | integer(optional) | Target feerate for the RBF transaction (in sat/vb).             |

#### Response

The response is the same as for [`createspend`](#createspend).

### `startrescan`

#### Request

| Field        | Type   | Description                                            |
| ------------ | ------ | ------------------------------------------------------ |
| `timestamp`  | int    | Date to start rescanning from, as a UNIX timestamp     |

#### Response

This command does not return anything for now.

| Field          | Type      | Description                                          |
| -------------- | --------- | ---------------------------------------------------- |

### `listconfirmed`

`listconfirmed` retrieves a paginated and ordered list of transactions that were confirmed within a given time window.
Confirmation time is based on the timestamp of blocks.

#### Request

| Field         | Type         | Description                                |
| ------------- | ------------ | ------------------------------------------ |
| `start`       | int          | Inclusive lower bound of the time window   |
| `end`         | int          | Inclusive upper bound of the time window   |
| `limit`       | int          | Maximum number of transactions to retrieve |

#### Response

| Field          | Type   | Description                                            |
| -------------- | ------ | ------------------------------------------------------ |
| `transactions` | array  | Array of [Transaction resource](#transaction-resource) |

##### Transaction Resource

| Field    | Type          | Description                                                               |
| -------- | ------------- | ------------------------------------------------------------------------- |
| `height` | int or `null` | Block height of the transaction, `null` if the transaction is unconfirmed |
| `time`   | int or `null` | Block time of the transaction, `null` if the transaction is unconfirmed   |
| `tx`     | string        | hex encoded bitcoin transaction                                           |

### `listtransactions`

`listtransactions` retrieves the transactions with the given txids.

#### Request

| Field         | Type            | Description                           |
| ------------- | --------------- | ------------------------------------- |
| `txids`       | array of string | Ids of the transactions  to retrieve  |

#### Response

| Field          | Type   | Description                                            |
| -------------- | ------ | ------------------------------------------------------ |
| `transactions` | array  | Array of [Transaction resource](#transaction-resource) |


### `createrecovery`

Create a transaction that sweeps all coins for which a timelocked recovery path is
currently available to a provided address with the provided feerate.

The `timelock` parameter can be used to specify which recovery path to use. By default,
we'll use the first recovery path available. If created for a later timelock a recovery
transaction may be satisfied using an earlier timelock but not the opposite.

Due to the fact coins are generally received at different block heights, not all coins may be
spendable through a single recovery path at the same time.

This command will error if no such coins are available or the sum of their value is not enough to
cover the requested feerate.

#### Request

| Field      | Type              | Description                                                                               |
| ---------- | ----------------- | ----------------------------------------------------------------------------------------- |
| `address`  | str               | The Bitcoin address to sweep the coins to.                                                |
| `feerate`  | integer           | Target feerate for the transaction, in satoshis per virtual byte.                         |
| `timelock` | int or `null`     | Recovery path to be used, identified by the number of blocks after which it is available. |

#### Response

| Field          | Type      | Description                                          |
| -------------- | --------- | ---------------------------------------------------- |
| `psbt`         | string    | PSBT of the recovery transaction, encoded as base64. |

### `updatelabels`

Update the labels from a given map of key/value, with the labelled bitcoin addresses, txids and
outpoints as keys and the label as value. If a label already exists for the given item, the new label
overrides the previous one. If a `null` value is passed, the label is deleted.

#### Request

| Field    | Type   | Description                                                                                                           |
| -------- | ------ | --------------------------------------------------------------------------------------------------------------------- |
| `labels` | object | A mapping from an item to be labelled (an address, a txid or an outpoint) to a label string (at most 100 chars long). |

### `getlabels`

Retrieve a map of items and their respective labels from a list of addresses, txids and outpoints.
Items without labels are not present in the response map.

#### Request

| Field   | Type         | Description                                                    |
| --------| ------------ | -------------------------------------------------------------- |
| `items` | string array | Items (address, txid or outpoint) of which to fetch the label. | 

#### Response

| Field    | Type   | Description                                                                      |
| -------- | ------ | -------------------------------------------------------------------------------- |
| `labels` | object | A mapping of bitcoin addresses, txids and outpoints as keys, and string as values |
