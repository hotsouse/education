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

## Task 03 — Escrow (Token-2022)

`initialize_escrow(seed, amount)` creates a unique state PDA derived from `["escrow", maker, seed]`, creates a dedicated token vault, records immutable maker/taker/mint/vault/refund/destination addresses, and deposits tokens with `transfer_checked`.

State machine: `Active (0) -> Released (1)` or `Active (0) -> Cancelled (2)`. Terminal states cannot be finalized again. `release_escrow` pays the precommitted taker token account; `cancel_escrow` refunds the precommitted maker token account. Both require the maker's signature, perform checked transfers with PDA signer seeds, and close the empty vault to refund its rent to the maker. The escrow state remains allocated to retain a permanent terminal marker.

### Threat model / security invariants

- Unauthorized callers cannot release or cancel: maker signature and PDA seeds are checked on-chain.
- Account substitution is blocked by checking the escrow's committed mint, vault and destination/refund account addresses.
- Cross-escrow vault sharing is prevented by binding the vault pubkey to a unique escrow state PDA; each initialized escrow creates a fresh vault account.
- Replay and double payout are prevented by terminal status checks and vault closure. The same maker/seed pair cannot be reused while the escrow state exists.
- Transfers use `token_interface::transfer_checked`, preserving Token-2022 mint and decimal verification.
- Unsupported transfer-fee / transfer-hook mint extensions may cause instructions to fail; this implementation is intended for standard Token-2022 mints without such extensions. Do not use with confidential or fee-bearing tokens without extension-aware accounting.
- No secret keys or seed phrases are needed in source control. Keep keypairs and `.env` out of Git.

### Build and test

Run `anchor build`, then `cargo test` using the pinned toolchain. This uploaded starter includes its existing LiteSVM test only; full escrow positive and negative end-to-end LiteSVM coverage must be added and executed before submission. **Do not claim the task is fully verified without these tests.**

## Task 03 escrow validation (Token-2022)

The escrow PDA is derived from `["escrow", maker, seed_le_u64]` and owns a unique
vault PDA derived from `["vault", escrow]`. The maker deposits via
`transfer_checked`. Release transfers the committed amount to the taker's
committed token account; cancellation refunds the maker's committed token
account. Both close the vault and mark the escrow state finalized. The escrow
state is intentionally retained as a replay-prevention tombstone, so its rent
is not refunded.

### State transitions

`Uninitialized -> Active -> Released` or `Uninitialized -> Active -> Cancelled`.
No transition is permitted out of a terminal state. Both finalization paths
currently require the maker's signature; this is a maker-controlled escrow,
not a trustless two-party settlement protocol.

### Threat model and limitations

The program validates PDA derivation, maker signature, token program, mint,
committed recipient/refund address, active state and exact vault balance.
It uses Token-2022 interface CPIs with checked decimals. The current version
assumes ordinary Token-2022 mints without transfer-fee, transfer-hook or other
extensions that alter transfer semantics. A token account holding unexpected
extra tokens cannot be finalized until reconciled. Do not use for real funds
without independent audit and a documented policy for such extensions.

### Tests

From the Anchor workspace root:

```sh
anchor build
cargo test
```

`tests/escrow.rs` exercises LiteSVM release, cancel, token balances, vault
closure, duplicate finalization, zero amount, duplicate seed, wrong signer,
and wrong destination. Tests require the built `.so` in `target/deploy`.
