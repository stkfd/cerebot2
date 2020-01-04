import {ComponentProps, ReactNode} from "react";
import "./button.css";

interface Props extends ComponentProps<'button'> {
    outline?: boolean,
}

const Button = (props: Props) => {
    return <>
        <button
            className="btn" {...props}
        >
            {props.children}
        </button>
    </>;
};

export default Button;
