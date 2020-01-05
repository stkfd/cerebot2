import "./navigation.css";
import Link from "next/link";
import {FunctionComponent} from "react";

const TopBar: FunctionComponent = () => (
    <div className="flex mb-6 py-4 w-full justify-center bg-primary-600">
        <div className="flex w-2/3 items-center">
            <h1><Link href="/"><a className="w-1/6 text-2xl">cerebot</a></Link></h1>
            <ul className="menu">
                <li>
                    <Link href="/doc">
                        <a>API</a>
                    </Link>
                </li>
                <li>
                    <Link href="/channels">
                        <a>Channels</a>
                    </Link>
                </li>
                <li>
                    <Link href="/commands">
                        <a>Commands</a>
                    </Link>
                </li>
            </ul>
        </div>
    </div>
);

export default TopBar;