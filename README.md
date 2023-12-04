# TWML

Tailwind Markup Language is a [Markup Language](https://en.wikipedia.org/wiki/Markup_language)
that seeks to convert [TailwindCSS](https://tailwindcss.com/) into a framework for writing documents.

The syntax is inspired by [Haml](https://haml.info/) and acts as an abstracted version of HTML,
allowing for quick meta-programming.

In essence, writing TWML is like writing a website, but with fancy syntax and built-in support for TailwindCSS
using [Railwind](https://github.com/pintariching/railwind).

Although designed for websites, TailwindCSS is simple and flexible, making it perfect for documents.

_TWML converts documents to PDF using [rust-headless-chrome](https://github.com/rust-headless-chrome/rust-headless-chrome)._

```
\\ Example Document in TWML
\div.page.p-14
    \div.flex.h-1/5
        \div.w-1/2
            \p Lorem ipsum dolor sit amet, consectetur adipiscing elit.
        \div.text-right
            \p.font-bold Finance Department
            \span
                Donec rutrum consequat tortor sed elementum.
                Phasellus viverra nulla a nisi cursus viverra.
                In in massa at massa accumsan mattis.
                In varius tortor odio, in posuere nunc sagittis vitae.

    \p December 2023
    \p.text-center.font-bold Duis accumsan accumsan augue in iaculis.
    \span
        Hello Human,

        Suspendisse ut ullamcorper risus. Nam sed urna ut ligula vestibulum
        venenatis nec quis ligula. Vestibulum ante ipsum primis in faucibus
        orci luctus et ultrices posuere cubilia curae; Duis sit amet pulvinar
        risus, at mollis ex. Nam eu tristique leo. Nulla interdum lorem nunc,
        sit amet condimentum sem iaculis a. In lorem lectus, molestie eget
        ligula nec, eleifend mattis purus. Praesent suscipit sem sed ante
        tempor consectetur. Class aptent taciti sociosqu ad litora torquent
        per conubia nostra, per inceptos himenaeos. Aenean vitae massa ut
        augue blandit tempor. Integer vehicula nunc non ligula congue scelerisque.
        Sed mattis et nulla venenatis luctus. Integer rutrum maximus tincidunt.

        \p.font-bold Thank you for checking out TWML!

\div.page.p-14
    New page - Fresh start.
```

## Reference

| Description         | Example                                      | Note                                                                 |
| ------------------- | -------------------------------------------- | -------------------------------------------------------------------- |
| Comments            | `\\ Comment`                                 | Multiline comments are not supported                                 |
| Include file        | `@include file_path`                         | Top-level declaration; Make file accessible during rendering process |
| Page size           | `@page-width size_in_mm` (or `@page-height`) | Top-level declaration                                                |
| New page            | `\div.page`                                  | Builtin support for `page` class                                     |
| Simple content      | `\p Simple content`                          |                                                                      |
| Block content       | \p<br>`   `Block content                     |
| TailwindCSS Classes | `\p.class1.class2`                           |                                                                      |
| HTML Attributes     | `\p{key1="value" key2="value"}`              |

_This project was released within 2 days of creating it - odd behavior is to be expected._
