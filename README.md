# Solana Level 1 Token Starter

Учебный starter для итоговых заданий первого уровня курса Superteam KZ. Он показывает современный минимальный каркас токен-программы без привязки к legacy JavaScript SDK.

## Зафиксированный стек

- Anchor CLI и crates: `1.1.2`
- Solana CLI: `3.1.10`
- Rust: `1.89.0`
- тесты программ: Rust + LiteSVM `0.10.0`
- токены: `anchor_spl::token_interface`, совместимый с Token Program и Token-2022
- рекомендуемый клиент для нового TypeScript-кода: `@solana/kit`

`@solana/web3.js` относится к legacy-стеку. TypeScript-клиент Anchor `@anchor-lang/core` по-прежнему зависит от `@solana/web3.js` v1, поэтому в этом starter тесты написаны на Rust и LiteSVM. Для нового клиентского приложения используйте `@solana/kit`, если задание явно не требует другого.

Оригинальный Token Program остается рабочим и широко используется. Для новых токенов в учебных заданиях используйте Token-2022, а program-код пишите через `token_interface`, чтобы сохранить совместимость с обоими Token Program.

## Что уже реализовано

- создание mint с выбранной token-программой;
- создание associated token account;
- выпуск токенов через `mint_to`;
- перевод через `transfer_checked`;
- проверки положительной суммы, полномочий, mint и token program на уровне Anchor accounts constraints;
- один эталонный LiteSVM-тест создания Token-2022 mint.

Функции `burn_tokens` и Escrow намеренно отсутствуют: студент реализует их в следующих заданиях.

## Быстрый старт

1. Установите версии из раздела «Зафиксированный стек» через AVM, rustup и официальный Solana installer.
2. Создайте собственный локальный program keypair и выполните `anchor keys sync`. Не коммитьте keypair или seed phrase.
3. Выполните `anchor build`.
4. Выполните `anchor test` или `cargo test` после сборки программы.
5. Разрабатывайте каждое задание в отдельной ветке: `task/01-tests`, `task/02-burn`, `task/03-escrow`.

Тест загружает собранный файл `target/deploy/solana_level_1_token_starter.so`, поэтому перед первым `cargo test` нужен `anchor build`.

## Правила сдачи

- сдавайте публичную ссылку на GitHub-репозиторий и указывайте ветку или commit SHA;
- добавьте в README команды сборки и тестирования, ожидаемый результат и краткое описание архитектуры;
- не добавляйте в репозиторий private keys, seed phrases, `.env` с секретами или файлы keypair;
- не используйте `@solana/web3.js` в новом клиентском коде;
- для переводов токенов используйте `transfer_checked`, а не unchecked transfer;
- не подменяйте проверки полномочий только клиентской логикой: все критичные инварианты должны проверяться программой.

## Что считается современным решением

Современность здесь определяется не только номером версии. Решение должно использовать строгие account constraints, проверяемые state transitions, Token-2022 для нового токена, `token_interface` для совместимости, `transfer_checked` для переводов и воспроизводимые LiteSVM-тесты. Если официальные стабильные рекомендации Solana или Anchor изменятся, студент должен зафиксировать выбранные версии и объяснить отклонение в README.

## Task 02 — Safe token burning (`task/02-burn`)

`burn_tokens(amount: u64)` destroys exactly `amount` **base units** of the
signer's tokens, reducing both the source token-account balance and mint total
supply. `amount` must be positive. The program uses
`anchor_spl::token_interface::burn_checked` with `mint.decimals` read from the
validated on-chain mint (never supplied by the client).

### On-chain account constraints

| Account | Type | Enforcement |
|---|---|---|
| `authority` | `Signer` | Must sign; `token::authority = authority` ties it to the source owner |
| `mint` | `InterfaceAccount<Mint>` | `mint::token_program = token_program` |
| `source` | mutable `InterfaceAccount<TokenAccount>` | `token::mint = mint`, `token::authority = authority`, `token::token_program = token_program` |
| `token_program` | `Interface<TokenInterface>` | Accepts only supported SPL Token / Token-2022 programs; must own both validated token accounts |

Additional explicit guards: `amount > 0` (`AmountMustBePositive`) and
`source.amount >= amount` (`InsufficientTokenBalance`). The CPI uses
`BurnChecked`, not unchecked `Burn`. All validation runs on-chain; there is no
legacy JavaScript client and no critical `UncheckedAccount`.

### Build and test from a clean checkout

Install the exact versions specified above, then run from the repository root:

```bash
rustc --version        # rustc 1.89.0
solana --version       # solana-cli 3.1.10
anchor --version       # anchor-cli 1.1.2
anchor build
cargo test
```

`cargo test` reads `target/deploy/solana_level_1_token_starter.so`, so
**`anchor build` must run first**. The Rust LiteSVM tests use version `0.10.0`
and execute locally without a validator, RPC access, or wallet private keys.

`tests/burn_tokens.rs` verifies a successful burn reduces token balance and
mint supply by the identical amount; zero amount, wrong signer/authority,
missing signature, wrong mint, insufficient funds, and mismatched token program
all fail. Each negative case asserts that mint and token-account data and
lamports remain unchanged. The original `create_token` test remains present.

### Submission and security

Push the changes to `task/02-burn` **after Task 01 is accepted** and submit
`https://github.com/<your-user>/<your-repo>/tree/task/02-burn` (or a precise
commit SHA). Do not commit `target/`, `.keys/`, `.env`, private keypairs, seed
phrases, or wallet JSON files. See `.gitignore` and `SECURITY.md`.
