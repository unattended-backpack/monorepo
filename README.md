# Monorepo

This is the grand unified monorepo managing all Unattended Backpack infrastructure and project code. This repository is structured such that merges to `master` equate to automatic deployment of live [projects](#projects); please view each project for more specific details.

# Security

If you discover any bug; flaw; issue; dæmonic incursion; or other malicious, negligent, or incompetent action that impacts the security of any of these projects please responsibly disclose them to us; instructions are available [here](./SECURITY.md).

# License

The [license](./LICENSE) for all of our original work is `LicenseRef-VPL WITH AGPL-3.0-only`. This includes every asset in this repository: code, documentation, images, branding, and more. You are licensed to use all of it so long as you maintain _maximum possible virality_ and our copyleft licenses.

Permissive open source licenses are tools for the corporate subversion of libre software; visible source licenses are an even more malignant scourge. All original works in this project are to be licensed under the most aggressive, virulently-contagious copyleft terms possible. To that end everything is licensed under the [Viral Public License](./licenses/LicenseRef-VPL) coupled with the [GNU Affero General Public License v3.0](./licenses/AGPL-3.0-only) for use in the event that some unaligned party attempts to weasel their way out of copyleft protections. In short: if you use or modify anything in this project for any reason, your project must be licensed under these same terms.

For art assets specifically, in case you want to further split hairs or attempt to weasel out of this virality, we explicitly license those under the viral and copyleft [Free Art License 1.3](./licenses/FreeArtLicense-1.3).

# Projects

The modern internet is sterile and unfree. It is opaque and riddled with censorship. Our mission at Unattended Backpack is to build maximally-trustless software that restores user sovereignty. Working from the fundamentals, this leads us to conclude the following:
1. We need fully-composable and unstoppable compute.
2. A blockchain is best suited for this: software can exist immutably where anyone can interact with it.
3. The measure of unstoppability is a function of how trustless the underlying blockchain is.
4. Ethereum is by far, far, far, the more robust network of censorship-resistant compute.
5. Every other alt-L1 relying on consensus other than Ethereum is therefore immediately non-viable.
6. We need to make the trade-off of increased latency for less expensive execution and use an L2.
7. There is sadly no trustless L2 yet.
8. We can build the trustless L2 we want.
9. Then we can build everything else we want on the L2.

Therefore, we are building the following projects.
- [Priory](#priory): a Rust crate that stands on the shoulders of giants ([libp2p](https://libp2p.io/)) and makes managing a robust peer-to-peer gossip network easier.
- [Sigil](#sigil): a fully-trustless L2 that we can be proud of building on. Achieving this is our grandest of missions and unlocks incredible possibilities.

All of these projects are libre and copyleft; we welcome all [contributors](./CONTRIBUTING.md).

## Priory

Priory is a Rust crate for managing joining communication in a strongly-opinionated ([libp2p](https://libp2p.io/)) swarm. This is a thin wrapper around libp2p implementing some custom peer discovery techniques for achieving maximal connectivity across peers local, public, or holepunched into.

The Priory library, alongside its building and testing instructions, is located in the [`priory`](./priory/) folder.

## Sigil

Hermetic Qabalah is a flavor of Renaissance-era syncretic occultism developed by fusing Christian Cabala—a derivative of living Jewish Kabbalistic traditions—with medieval European alchemical practices. Within this Hermeticism, sigils emerge as unique visual seals which grant control over angelic or dæmonic entities. Four hundred years later, the modern practice of Chaos Magick adopts sigils. Under Chaos Magick, casters prepare sigils as the symbolic representation of their will to achieve some task, and unleash autonomous servitors.

We cannot help but make the comparison between sigils and Ethereum, our glorious and unstoppable world machine. A sigil is a commanding signature, whether forged in ink or cryptography. Sigils spawn autonomous and unstoppable agents, whether chained by the laws of dæmons or state transitions.

Like those sigils of old, our Sigil is an esoteric and forbidden tool. Our Sigil is a dark shadow at the edge of orthodoxy. Our Sigil is the bold and risky push for progress before the maws of a hungry abyss. Our Sigil has no training wheels, an exit window, and a trustless design.

Sigil is an Ethereum rollup with no centralized mechanisms. Sigil will save Ethereum.

The Sigil client, for participating in the rollup, is located in the [`sigil`](./sigil/) folder. That folder also includes useful instructions for building and testing the project.
