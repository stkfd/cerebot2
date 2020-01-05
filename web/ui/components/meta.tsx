import Head from 'next/head'
import "../styles/style.css"
import {FunctionComponent} from "react";

const Meta: FunctionComponent<{title: string}> = ({title}) => (
    <Head>
        <title>{title}</title>
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <meta charSet="utf-8" />
    </Head>
);

export default Meta;