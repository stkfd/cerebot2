import Head from 'next/head'
import "../styles/style.css"

export default ({title}) => (
    <Head>
        <title>{title}</title>
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <meta charSet="utf-8" />
    </Head>
);
