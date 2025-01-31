# finql

Purpose of this library is to provide over time a comprehensive toolbox for
quantitative analysis of financial assets in rust. The project is licensed under
Apache 2.0 or MIT license (see files LICENSE-Apache2.0 and LICENSE-MIT) at the
option of the user.

The goal is to provide a toolbox for pricing various financial products, like
bonds options or maybe even more complex products. In the near term, calculation
of the discounted cash flow value of bonds is in the focus, based on what
information is given by a standard prospect. Building blocks to achieve this
target include time periods (e.g. "3M" or "10Y"), bank holiday calendars,
business day adjustment rules, calculation of year fraction with respect to
typical day count convention methods, roll-out of cash flows, and calculating
valuation and risk figures like internal yield or duration that are useful for
an investor in these products. 

Functionality to calculate figures like fair values which are primarily
of interest in scenarios where one is fully hedged are not in the initial focus,
since an investor is by definition not fully hedged. Nevertheless, they might be
added later for comparison and estimating market prices.

## Structure

The Library is structured in a couple of sub-crates.

### finql-data

This crates contains definitions for basic data types, like Assets, Transactions,
Quotes, CashFlow, etc., some basic helper tools for dealing with currencies or
dates.

This crate also supports storing data, like market data, e.g. market quote
information, data related to portfolio and transaction management to be able to
support portfolio analysis (e.g. calculation of risk figures), and generic
storage of product details (e.g. bond specification). This is done by defining
data handler traits for various data categories, with concrete implementations
supporting storage in a database in separate crates.

### finql-sqlite

Implementation of database traits to be used with the `sqlite3` database,
supporting a file based database as well as in-memory databases.

### finql-postgres

Implementation of database traits to be used with a `postgreSQL` database.

## Examples

A couple of examples shall demonstrate different usages of the library.

### adding_period

A simple demo program that lets you add a time period, e.g `3M` for three months
or `-5Y` for 5 years before.

### uk_settlement_calendar

For any financial library, it is essential to be able to determine whether a 
specific date is a bank holiday, if the exchange is open interest could be paid.
A common approach is to just store a set of bank holidays. `finql` is different in 
that respect, as it defines a calendar as a set of rules how to calculate bank 
holidays. This examples demonstrates this by implementing the UK settlement 
calendar, which is particular interesting with its complex holiday rules, e.g. 
last Monday of August (Summer bank holiday), fixed day holidays that are moved
to the next working day if the holiday falls on a weekend, or singular holidays
which are not repeated every year, but celebrate unique events like a royal wedding.

### bond_comparison

This example demonstrate roll out of cash flows for fixed-rate investments by 
comparing two different bonds based on some key figures, e.g. yield to maturity.

### quotes_db

A demonstration for retrieving market quote prices and storing the results 
persistently in a database.

### transaction_db

Explores how to deal with transactions and how to store them in a database. 
Transactions mainly define the cash flows that occur over the lifetime of an
investment. In `finql`, a (variable) transactions are the building 
blocks for any analysis. In the end, the flow of cash flows is what makes the 
distinction between the success and failure. 

### total_return 

A demonstration of a certain strategy simulation. Here, we compare the outcome
of different investment strategy into a single stock. A static investment is 
compared with a strategy that re-invests any cash flow into the stock, either
with or without taking fee and tax payments into account. The results will be
plotted to an image, see for example [Total Return image](strategies.png)

## Market Data

Market data quotes can be fetched automatically from various vendors and stored into a database. 
Fetching the (real time) quote or a quote history is implemented for the vendors 
[yahoo! finance](https://finance.yahoo.com/),
[alpha vantage](https://www.alphavantage.co/), 
[gurufocus](https://www.gurufocus.com/new_index/) and 
[eodhistoricaldata](https://eodhistoricaldata.com/). Please note that all except yahoo! finance 
require a user token that is only provided after registration with the service. For gurufocus,
this required a paid license.

## Database setup
With version 0.8.x onwards, we use the sqlx crate, which supports compile time checks of SQL
queries. This, however, requires that the environment variable DATABASE_URL is set to the 
appropriate connection string. For Postgres, using a unix socket connection the variable
could be set to something like

```bash
export DATABASE_URL="postgresql:///<dbname>?user=<user>&password=<password>&ssl=false"
```

and for sqlite3 the string would look like

```bash
export DATABASE_URL="sqlite:memory"
```

or 

```bash
export DATABASE_URL="sqlite:<absolute_path>/finql-sqlite/data/test.db"
```

Alternatively, you could follow the instructions on 
https://docs.rs/sqlx/0.5.1/sqlx/macro.query.html#offline-mode-requires-the-offline-feature.
This requires some preparation, but without the necessity to have a live database connection. Try 
this approach also if you want to compile more than one database adapter (e.g. finql-sqlite 
and finql-postgres).
