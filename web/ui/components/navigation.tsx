import "./navigation.css";
import Link from "next/link";

const Navigation = () => (
    <div className="flex mb-4 py-4 w-full justify-center bg-primary-600">
        <div className="flex w-2/3 items-center">
            <h1><Link href="/"><a className="w-1/6 font-bold text-2xl">cerebot</a></Link></h1>
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

export default Navigation;